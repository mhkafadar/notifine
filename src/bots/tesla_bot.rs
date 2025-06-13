use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use chrono::{DateTime, Duration, Utc};
use diesel::prelude::*;
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use teloxide::{
    dispatching::{dialogue, dialogue::InMemStorage, UpdateHandler},
    macros::BotCommands,
    prelude::*,
};
use url::Url;
use uuid::Uuid;

use notifine::{
    crypto::TokenCrypto,
    models::{NewTeslaAuth, NewTeslaOrder, TeslaAuth, TeslaOrder},
    schema::{tesla_auth, tesla_orders},
};

type TeslaDialogue = Dialogue<State, InMemStorage<State>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    WaitingForAuthCode {
        code_verifier: String,
    },
}

fn get_redirect_uri() -> String {
    // Tesla's official void callback for third-party apps
    "https://auth.tesla.com/void/callback".to_string()
}
const AUTH_URL: &str = "https://auth.tesla.com/oauth2/v3/authorize";
const TOKEN_URL: &str = "https://auth.tesla.com/oauth2/v3/token";
const SCOPE: &str = "openid email offline_access";
const CODE_CHALLENGE_METHOD: &str = "S256";
const ORDERS_API_URL: &str = "https://owner-api.teslamotors.com/api/1/users/orders";
const ORDER_DETAILS_API_URL: &str = "https://akamai-apigateway-vfx.tesla.com/tasks";
const APP_VERSION: &str = "4.44.5-3304";

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

#[derive(Debug)]
pub struct PkceParams {
    pub code_verifier: String,
    pub code_challenge: String,
}

impl PkceParams {
    pub fn generate() -> Self {
        let code_verifier = generate_code_verifier();
        let code_challenge = generate_code_challenge(&code_verifier);

        Self {
            code_verifier,
            code_challenge,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    #[serde(rename = "referenceNumber")]
    pub reference_number: String,
    #[serde(rename = "orderStatus")]
    pub order_status: String,
    #[serde(rename = "modelCode")]
    pub model_code: String,
    pub vin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdersResponse {
    pub response: Vec<Order>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderSnapshot {
    pub order_id: String,
    pub status: String,
    pub model: String,
    pub vin: Option<String>,
    pub reservation_date: Option<String>,
    pub order_booked_date: Option<String>,
    pub vehicle_odometer: Option<f64>,
    pub odometer_type: Option<String>,
    pub routing_location: Option<u64>,
    pub delivery_window: Option<String>,
    pub eta_to_delivery_center: Option<String>,
    pub delivery_appointment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OrderChange {
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub change_type: ChangeType,
    pub context_message: String,
}

#[derive(Debug, Clone)]
pub enum ChangeType {
    Added,
    Removed,
    Modified,
}

fn get_token_crypto() -> Result<TokenCrypto> {
    let key = std::env::var("TESLA_ENCRYPTION_KEY")
        .map_err(|_| anyhow::anyhow!("TESLA_ENCRYPTION_KEY not set"))?;
    TokenCrypto::new(&key)
}

pub fn format_date(date_str: &str) -> String {
    // Try to parse the ISO date string with timezone first
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        // Format as "28 May 2025 18:30:56"
        dt.format("%d %B %Y %H:%M:%S").to_string()
    } else {
        // Try to parse as UTC datetime without timezone suffix
        // Check if it already has timezone info (Z, +, or - after T followed by time)
        let has_timezone = date_str.contains('Z')
            || (date_str.contains('T')
                && (date_str.matches('+').count() > 0 || date_str.matches('-').count() > 2));

        let date_with_z = if has_timezone {
            date_str.to_string()
        } else {
            format!("{}Z", date_str)
        };

        if let Ok(dt) = DateTime::parse_from_rfc3339(&date_with_z) {
            dt.format("%d %B %Y %H:%M:%S").to_string()
        } else {
            // If parsing still fails, return the original string
            date_str.to_string()
        }
    }
}

pub fn generate_code_verifier() -> String {
    let random_bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen()).collect();
    general_purpose::URL_SAFE_NO_PAD.encode(&random_bytes)
}

pub fn generate_code_challenge(code_verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

pub fn generate_auth_url(pkce_params: &PkceParams) -> String {
    let state = Uuid::new_v4().to_string();
    let redirect_uri = get_redirect_uri();

    log::info!("AUTH_URL: Generating auth URL with:");
    log::info!("AUTH_URL: - client_id: ownerapi");
    log::info!("AUTH_URL: - redirect_uri: {}", redirect_uri);
    log::info!("AUTH_URL: - scope: {}", SCOPE);
    log::info!(
        "AUTH_URL: - code_challenge_method: {}",
        CODE_CHALLENGE_METHOD
    );
    log::info!("AUTH_URL: - state: {}", state);

    let mut auth_url = Url::parse(AUTH_URL).unwrap();
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", "ownerapi")
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("state", &state)
        .append_pair("code_challenge", &pkce_params.code_challenge)
        .append_pair("code_challenge_method", CODE_CHALLENGE_METHOD);

    let final_url = auth_url.to_string();
    log::info!("AUTH_URL: Generated URL: {}", final_url);
    final_url
}

async fn exchange_code_for_tokens(
    client: &Client,
    auth_code: &str,
    code_verifier: &str,
) -> Result<TokenResponse> {
    let redirect_uri = get_redirect_uri();
    let mut token_data = HashMap::new();
    token_data.insert("grant_type", "authorization_code");
    token_data.insert("client_id", "ownerapi");
    token_data.insert("code", auth_code);
    token_data.insert("redirect_uri", &redirect_uri);
    token_data.insert("code_verifier", code_verifier);

    log::info!(
        "TOKEN_EXCHANGE: Sending token request with data: {:?}",
        token_data
    );

    let response = client.post(TOKEN_URL).form(&token_data).send().await?;

    log::info!(
        "TOKEN_EXCHANGE: Received response with status: {}",
        response.status()
    );

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Could not read error response".to_string());
        log::error!(
            "TOKEN_EXCHANGE: Failed with status {} - Error: {}",
            status,
            error_text
        );
        return Err(anyhow::anyhow!(
            "Token exchange failed with status: {} - Error: {}",
            status,
            error_text
        ));
    }

    let token_response: TokenResponse = response.json().await?;
    log::info!("TOKEN_EXCHANGE: Successfully received tokens");
    Ok(token_response)
}

pub async fn refresh_tokens(client: &Client, refresh_token: &str) -> Result<TokenResponse> {
    let mut token_data = HashMap::new();
    token_data.insert("grant_type", "refresh_token");
    token_data.insert("client_id", "ownerapi");
    token_data.insert("refresh_token", refresh_token);

    let response = client.post(TOKEN_URL).form(&token_data).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Token refresh failed with status: {}",
            response.status()
        ));
    }

    let token_response: TokenResponse = response.json().await?;
    Ok(token_response)
}

pub fn is_token_valid(access_token: &str) -> Result<bool> {
    let parts: Vec<&str> = access_token.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow::anyhow!("Invalid JWT format"));
    }

    let payload = parts[1];
    let padded_payload = match payload.len() % 4 {
        0 => payload.to_string(),
        n => format!("{}{}", payload, "=".repeat(4 - n)),
    };

    let decoded = general_purpose::STANDARD.decode(padded_payload)?;
    let jwt_payload: serde_json::Value = serde_json::from_slice(&decoded)?;

    if let Some(exp) = jwt_payload.get("exp").and_then(|v| v.as_u64()) {
        let current_time = chrono::Utc::now().timestamp() as u64;
        Ok(exp > current_time)
    } else {
        Err(anyhow::anyhow!("Token expiration not found"))
    }
}

pub async fn retrieve_orders(client: &Client, access_token: &str) -> Result<Vec<Order>> {
    let response = client
        .get(ORDERS_API_URL)
        .bearer_auth(access_token)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to retrieve orders: {}",
            response.status()
        ));
    }

    let orders_response: OrdersResponse = response.json().await?;
    Ok(orders_response.response)
}

pub async fn get_order_details(
    client: &Client,
    order_id: &str,
    access_token: &str,
) -> Result<serde_json::Value> {
    let url = format!(
        "{}?deviceLanguage=en&deviceCountry=DE&referenceNumber={}&appVersion={}",
        ORDER_DETAILS_API_URL, order_id, APP_VERSION
    );

    let response = client.get(&url).bearer_auth(access_token).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to retrieve order details: {}",
            response.status()
        ));
    }

    let details: serde_json::Value = response.json().await?;
    Ok(details)
}

pub fn create_order_snapshot(order: &Order, details: &serde_json::Value) -> OrderSnapshot {
    let mut snapshot = OrderSnapshot {
        order_id: order.reference_number.clone(),
        status: order.order_status.clone(),
        model: order.model_code.clone(),
        vin: order.vin.clone(),
        reservation_date: None,
        order_booked_date: None,
        vehicle_odometer: None,
        odometer_type: None,
        routing_location: None,
        delivery_window: None,
        eta_to_delivery_center: None,
        delivery_appointment: None,
    };

    if let Some(tasks) = details.get("tasks") {
        // Extract reservation details
        if let Some(registration) = tasks.get("registration") {
            if let Some(order_details) = registration.get("orderDetails") {
                snapshot.reservation_date = order_details
                    .get("reservationDate")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                snapshot.order_booked_date = order_details
                    .get("orderBookedDate")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                snapshot.vehicle_odometer = order_details
                    .get("vehicleOdometer")
                    .and_then(|v| v.as_f64());

                snapshot.odometer_type = order_details
                    .get("vehicleOdometerType")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                snapshot.routing_location = order_details
                    .get("vehicleRoutingLocation")
                    .and_then(|v| v.as_u64());
            }
        }

        // Extract scheduling details
        if let Some(scheduling) = tasks.get("scheduling") {
            snapshot.delivery_window = scheduling
                .get("deliveryWindowDisplay")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            snapshot.delivery_appointment = scheduling
                .get("apptDateTimeAddressStr")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        // Extract ETA details
        if let Some(final_payment) = tasks.get("finalPayment") {
            if let Some(data) = final_payment.get("data") {
                snapshot.eta_to_delivery_center = data
                    .get("etaToDeliveryCenter")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
    }

    snapshot
}

pub fn format_order_changes(
    order_reference: &str,
    changes: &[OrderChange],
    new_snapshot: &OrderSnapshot,
) -> String {
    let mut message = String::new();

    // Find the most significant change and create a header
    // Priority: Status > VIN > Odometer > Location > Date
    for change in changes {
        match change.field.as_str() {
            "Status" => {
                message.push_str(&format!(
                    "📊 Order Status Updated: {}\n",
                    change.context_message
                ));
                break;
            }
            "VIN" => {
                message.push_str(&format!("🎯 VIN Assignment: {}\n", change.context_message));
                break;
            }
            "Vehicle Odometer" => {
                message.push_str(&format!(
                    "🚗 Vehicle Movement Detected: {}\n",
                    change.context_message
                ));
                break;
            }
            "Delivery Center" => {
                message.push_str(&format!(
                    "🏢 Delivery Location Changed: {}\n",
                    change.context_message
                ));
                break;
            }
            "Delivery Window" | "ETA to Delivery Center" => {
                message.push_str(&format!(
                    "📅 Delivery Schedule Updated: {}\n",
                    change.context_message
                ));
                break;
            }
            _ => {}
        }
    }

    // Add git diff style changes
    message.push('\n');
    for change in changes {
        match change.change_type {
            ChangeType::Modified => {
                if let (Some(old_val), Some(new_val)) = (&change.old_value, &change.new_value) {
                    message.push_str(&format!("🔴 - {}: {}\n", change.field, old_val));
                    message.push_str(&format!("🟢 + {}: {}\n", change.field, new_val));
                }
            }
            ChangeType::Added => {
                if let Some(new_val) = &change.new_value {
                    message.push_str(&format!("🟢 + {}: {}\n", change.field, new_val));
                }
            }
            ChangeType::Removed => {
                if let Some(old_val) = &change.old_value {
                    message.push_str(&format!("🔴 - {}: {}\n", change.field, old_val));
                }
            }
        }
    }
    message.push_str(&format!("\n{}\n\n", "=".repeat(50)));

    // Order Details
    message.push_str("📋 Order Details:\n");
    message.push_str(&format!("- Order ID: {}\n", order_reference));
    message.push_str(&format!("- Status: {}\n", new_snapshot.status));
    message.push_str(&format!("- Model: {}\n", new_snapshot.model));
    message.push_str(&format!(
        "- VIN: {}\n\n",
        new_snapshot.vin.as_deref().unwrap_or("N/A")
    ));

    // Reservation Details
    if new_snapshot.reservation_date.is_some() || new_snapshot.order_booked_date.is_some() {
        message.push_str("📅 Reservation Details:\n");
        if let Some(res_date) = &new_snapshot.reservation_date {
            message.push_str(&format!("- Reservation Date: {}\n", format_date(res_date)));
        }
        if let Some(book_date) = &new_snapshot.order_booked_date {
            message.push_str(&format!(
                "- Order Booked Date: {}\n",
                format_date(book_date)
            ));
        }
        message.push('\n');
    }

    // Vehicle Status
    if new_snapshot.vehicle_odometer.is_some() {
        message.push_str("🚙 Vehicle Status:\n");
        if let Some(odometer) = new_snapshot.vehicle_odometer {
            let odo_type = new_snapshot.odometer_type.as_deref().unwrap_or("KM");
            message.push_str(&format!(
                "- Vehicle Odometer: {:.2} {}\n",
                odometer, odo_type
            ));
        }
        message.push('\n');
    }

    // Delivery Information
    message.push_str("🚚 Delivery Information:\n");
    if let Some(routing) = new_snapshot.routing_location {
        message.push_str(&format!("- Routing Location: {}\n", routing));
    }
    if let Some(window) = &new_snapshot.delivery_window {
        message.push_str(&format!("- Delivery Window: {}\n", window));
    }
    if let Some(eta) = &new_snapshot.eta_to_delivery_center {
        message.push_str(&format!("- ETA to Delivery Center: {}\n", format_date(eta)));
    }
    if let Some(appointment) = &new_snapshot.delivery_appointment {
        message.push_str(&format!(
            "- Delivery Appointment: {}\n",
            format_date(appointment)
        ));
    } else {
        message.push_str("- Delivery Appointment: N/A\n");
    }

    message
}

pub fn compare_orders(
    old_snapshot: &OrderSnapshot,
    new_snapshot: &OrderSnapshot,
) -> Vec<OrderChange> {
    let mut changes = Vec::new();

    // Status changes
    if old_snapshot.status != new_snapshot.status {
        let context_message = match new_snapshot.status.as_str() {
            "DELIVERED" => "🎉 Congratulations! Your Tesla has been delivered!".to_string(),
            "READY_FOR_DELIVERY" => "🚗 Great news! Your Tesla is ready for delivery!".to_string(),
            "IN_TRANSIT" => "🚛 Your Tesla is on its way!".to_string(),
            "PRODUCTION" => "🏭 Your Tesla is in production!".to_string(),
            _ => format!(
                "📋 Order status changed from {} to {}",
                old_snapshot.status, new_snapshot.status
            ),
        };

        changes.push(OrderChange {
            field: "Status".to_string(),
            old_value: Some(old_snapshot.status.clone()),
            new_value: Some(new_snapshot.status.clone()),
            change_type: ChangeType::Modified,
            context_message,
        });
    }

    // VIN changes
    if old_snapshot.vin != new_snapshot.vin {
        let context_message = if new_snapshot.vin.is_some() && old_snapshot.vin.is_none() {
            "🎯 Exciting! Your Tesla has been assigned a VIN!".to_string()
        } else {
            "🔧 VIN information updated".to_string()
        };

        changes.push(OrderChange {
            field: "VIN".to_string(),
            old_value: old_snapshot.vin.clone(),
            new_value: new_snapshot.vin.clone(),
            change_type: ChangeType::Modified,
            context_message,
        });
    }

    // Odometer changes
    if old_snapshot.vehicle_odometer != new_snapshot.vehicle_odometer {
        let context_message = if let (Some(old_odo), Some(new_odo)) =
            (old_snapshot.vehicle_odometer, new_snapshot.vehicle_odometer)
        {
            if new_odo > old_odo {
                "🚗💨 Hey! Your vehicle has moved!".to_string()
            } else {
                "📊 Vehicle odometer updated".to_string()
            }
        } else if new_snapshot.vehicle_odometer.is_some() {
            "📊 Vehicle odometer information added".to_string()
        } else {
            "📊 Vehicle odometer information updated".to_string()
        };

        changes.push(OrderChange {
            field: "Vehicle Odometer".to_string(),
            old_value: old_snapshot.vehicle_odometer.map(|v| format!("{:.2}", v)),
            new_value: new_snapshot.vehicle_odometer.map(|v| format!("{:.2}", v)),
            change_type: ChangeType::Modified,
            context_message,
        });
    }

    // Delivery window changes
    if old_snapshot.delivery_window != new_snapshot.delivery_window {
        let context_message = match (&old_snapshot.delivery_window, &new_snapshot.delivery_window) {
            (Some(_), Some(_)) => "📅 Your delivery window has been updated".to_string(),
            (None, Some(_)) => "📅 Delivery window has been set!".to_string(),
            (Some(_), None) => "📅 Delivery window information updated".to_string(),
            (None, None) => "📅 Delivery window updated".to_string(),
        };

        changes.push(OrderChange {
            field: "Delivery Window".to_string(),
            old_value: old_snapshot.delivery_window.clone(),
            new_value: new_snapshot.delivery_window.clone(),
            change_type: ChangeType::Modified,
            context_message,
        });
    }

    // ETA changes
    if old_snapshot.eta_to_delivery_center != new_snapshot.eta_to_delivery_center {
        let context_message = match (
            &old_snapshot.eta_to_delivery_center,
            &new_snapshot.eta_to_delivery_center,
        ) {
            (Some(_), Some(_)) => "📅 ETA to delivery center updated".to_string(),
            (None, Some(_)) => "📅 ETA to delivery center has been set!".to_string(),
            _ => "📅 ETA information updated".to_string(),
        };

        changes.push(OrderChange {
            field: "ETA to Delivery Center".to_string(),
            old_value: old_snapshot.eta_to_delivery_center.clone(),
            new_value: new_snapshot.eta_to_delivery_center.clone(),
            change_type: ChangeType::Modified,
            context_message,
        });
    }

    // Routing location changes
    if old_snapshot.routing_location != new_snapshot.routing_location {
        let context_message = "🏢 Delivery center has changed!".to_string();

        changes.push(OrderChange {
            field: "Delivery Center".to_string(),
            old_value: old_snapshot.routing_location.map(|v| v.to_string()),
            new_value: new_snapshot.routing_location.map(|v| v.to_string()),
            change_type: ChangeType::Modified,
            context_message,
        });
    }

    changes
}

async fn edit_order(bot: Bot, msg: Message, args: String) -> Result<()> {
    let chat_id = msg.chat.id;

    // Parse arguments
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        bot.send_message(
            chat_id,
            "❌ Please specify a field to edit.\n\n\
            Examples:\n\
            /edit km (reduces by 10km)\n\
            /edit km 150.5\n\
            /edit location (sets to Paris)\n\
            /edit location 12345\n\
            /edit date (sets to 1 week earlier)\n\
            /edit date 'Jan 15-22, 2024'",
        )
        .await?;
        return Ok(());
    }

    let field = parts[0].to_string();
    let value = if parts.len() > 1 {
        Some(parts[1..].join(" "))
    } else {
        None
    };

    // Check if user is authenticated
    let mut conn = notifine::establish_connection();
    let _auth = match tesla_auth::table
        .filter(tesla_auth::chat_id.eq(chat_id.0))
        .first::<TeslaAuth>(&mut conn)
        .optional()?
    {
        Some(auth) => auth,
        None => {
            bot.send_message(
                chat_id,
                "You need to login first! Use /login to authenticate.",
            )
            .await?;
            return Ok(());
        }
    };

    // Get existing order data
    let existing_order = tesla_orders::table
        .filter(tesla_orders::chat_id.eq(chat_id.0))
        .first::<TeslaOrder>(&mut conn)
        .optional()?;

    if existing_order.is_none() {
        bot.send_message(
            chat_id,
            "No order data found. Please run /orderstatus first to load your orders.",
        )
        .await?;
        return Ok(());
    }

    let order_data = existing_order.unwrap();
    let mut snapshots: Vec<OrderSnapshot> = serde_json::from_value(order_data.order_data.clone())?;

    // For simplicity, we'll edit the first order. In a full implementation,
    // you might want to allow users to specify which order to edit
    if let Some(snapshot) = snapshots.first_mut() {
        let success_message;

        match field.to_lowercase().as_str() {
            "km" => {
                let km_value = if let Some(val) = value {
                    match val.parse::<f64>() {
                        Ok(km) => km,
                        Err(_) => {
                            bot.send_message(
                                chat_id,
                                "❌ Invalid km value. Please provide a number (e.g., /edit km 150.5)",
                            )
                            .await?;
                            return Ok(());
                        }
                    }
                } else {
                    // Default: reduce current odometer by 10 km if it exists
                    let current_km = snapshot.vehicle_odometer.unwrap_or(100.0);
                    (current_km - 10.0).max(0.0)
                };

                snapshot.vehicle_odometer = Some(km_value);
                success_message = format!("✅ Vehicle odometer set to {:.2} km", km_value);
            }
            "location" => {
                let has_value = value.is_some();
                let location_value = if let Some(val) = value {
                    match val.parse::<u64>() {
                        Ok(loc) => loc,
                        Err(_) => {
                            bot.send_message(
                                chat_id,
                                "❌ Invalid location ID. Please provide a number (e.g., /edit location 12345)",
                            )
                            .await?;
                            return Ok(());
                        }
                    }
                } else {
                    // Default: Set to Paris Tesla Service Center (fictional ID)
                    75001 // Paris postal code as location ID
                };

                snapshot.routing_location = Some(location_value);
                success_message = if !has_value {
                    "✅ Delivery location set to Paris (default)".to_string()
                } else {
                    format!("✅ Delivery location set to {}", location_value)
                };
            }
            "date" | "delivery" => {
                let date_value = value.unwrap_or_else(|| {
                    // Default: Set delivery to one week earlier
                    let new_date = Utc::now() - Duration::days(7);
                    new_date.format("%b %d-%d, %Y").to_string()
                });

                snapshot.delivery_window = Some(date_value.clone());
                snapshot.delivery_appointment = Some(date_value.clone());
                success_message = format!("✅ Delivery date set to '{}'", date_value);
            }
            _ => {
                bot.send_message(
                    chat_id,
                    "❌ Unknown field. Supported fields: km, location, date\n\n\
                    Examples:\n\
                    /edit km (reduces by 10km)\n\
                    /edit km 150.5\n\
                    /edit location (sets to Paris)\n\
                    /edit location 12345\n\
                    /edit date (sets to 1 week earlier)\n\
                    /edit date 'Jan 15-22, 2024'",
                )
                .await?;
                return Ok(());
            }
        }

        // Save updated order data
        let updated_order_data = serde_json::to_value(&snapshots)?;
        diesel::update(tesla_orders::table.filter(tesla_orders::id.eq(order_data.id)))
            .set((
                tesla_orders::order_data.eq(&updated_order_data),
                tesla_orders::updated_at.eq(diesel::dsl::now),
            ))
            .execute(&mut conn)?;

        bot.send_message(
            chat_id,
            format!(
                "{}\n\n🔄 Run /orderstatus to see the simulated changes in action!",
                success_message
            ),
        )
        .await?;
    } else {
        bot.send_message(
            chat_id,
            "No orders found to edit. Please run /orderstatus first.",
        )
        .await?;
    }

    Ok(())
}

pub fn schema() -> UpdateHandler<anyhow::Error> {
    use dptree::case;

    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(case![Command::Start].endpoint(start))
        .branch(case![Command::OrderStatus].endpoint(order_status))
        .branch(case![Command::Login].endpoint(login))
        .branch(case![Command::Logout].endpoint(logout))
        .branch(case![Command::Edit { args }].endpoint(edit_order))
        .branch(case![Command::EnableMonitoring].endpoint(enable_monitoring))
        .branch(case![Command::DisableMonitoring].endpoint(disable_monitoring))
        .branch(case![Command::MonitoringStatus].endpoint(monitoring_status))
        .branch(case![Command::Teslacron].endpoint(teslacron));

    let message_handler = Update::filter_message()
        .branch(command_handler)
        .branch(case![State::WaitingForAuthCode { code_verifier }].endpoint(receive_auth_url));

    let callback_query_handler = Update::filter_callback_query().endpoint(handle_callback);

    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(message_handler)
        .branch(callback_query_handler)
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Tesla bot commands:")]
enum Command {
    #[command(description = "Start the bot")]
    Start,
    #[command(description = "Get your Tesla order status")]
    OrderStatus,
    #[command(description = "Login to Tesla account")]
    Login,
    #[command(description = "Logout from Tesla account")]
    Logout,
    #[command(description = "Edit order values")]
    Edit { args: String },
    #[command(description = "Enable automatic order monitoring")]
    EnableMonitoring,
    #[command(description = "Disable automatic order monitoring")]
    DisableMonitoring,
    #[command(description = "Check monitoring status")]
    MonitoringStatus,
    #[command(
        description = "Set Tesla monitoring interval in seconds (admin only). Usage: /teslacron <seconds>"
    )]
    Teslacron,
}

async fn start(bot: Bot, dialogue: TeslaDialogue, msg: Message) -> Result<()> {
    bot.send_message(
        msg.chat.id,
        "Welcome to Tesla Order Status Bot! 🚗\n\n\
        Features:\n\
        • Check your Tesla order status\n\
        • Automatic monitoring (checks every 5 minutes)\n\
        • Get notified only when something changes\n\n\
        Commands:\n\
        • /login - Authenticate with Tesla\n\
        • /orderstatus - Check order status\n\
        • /enablemonitoring - Turn on auto-monitoring\n\
        • /disablemonitoring - Turn off auto-monitoring\n\
        • /monitoringstatus - Check monitoring status\n\
        • /logout - Remove authentication\n\n\
        Start by using /login to authenticate!",
    )
    .await?;
    dialogue.update(State::Start).await?;
    Ok(())
}

async fn login(bot: Bot, dialogue: TeslaDialogue, msg: Message) -> Result<()> {
    let chat_id = msg.chat.id;
    log::info!("LOGIN: User {} initiated login process", chat_id);

    // Check if already authenticated with valid token
    let mut conn = notifine::establish_connection();
    let existing_auth = tesla_auth::table
        .filter(tesla_auth::chat_id.eq(chat_id.0))
        .first::<TeslaAuth>(&mut conn)
        .optional()?;

    if let Some(auth) = existing_auth {
        // Decrypt and check if the existing token is still valid
        let crypto = get_token_crypto()?;
        let decrypted_access_token = crypto.decrypt(&auth.access_token).unwrap_or_default();

        if is_token_valid(&decrypted_access_token).unwrap_or(false) {
            log::info!(
                "LOGIN: User {} already authenticated with valid token",
                chat_id
            );
            bot.send_message(
                chat_id,
                "You are already logged in! Use /orderstatus to check your orders.",
            )
            .await?;
            return Ok(());
        } else {
            // Token is invalid, delete it and proceed with new login
            log::info!("LOGIN: User {} has invalid token, clearing auth", chat_id);
            diesel::delete(tesla_auth::table.filter(tesla_auth::chat_id.eq(chat_id.0)))
                .execute(&mut conn)?;
        }
    }

    // Generate PKCE parameters
    let pkce_params = PkceParams::generate();
    let auth_url = generate_auth_url(&pkce_params);
    log::info!(
        "LOGIN: Generated auth URL for user {}: {}",
        chat_id,
        auth_url
    );

    // Send the auth URL for manual login
    bot.send_message(
        chat_id,
        format!(
            "🚗 Tesla Account Login\n\n\
            Please follow these steps:\n\n\
            1️⃣ Click this link to open Tesla login in your browser:\n{}\n\n\
            2️⃣ Log in with your Tesla account credentials\n\n\
            3️⃣ After login, you'll be redirected to a page that shows \"Page Not Found\" - this is normal!\n\n\
            4️⃣ Copy the entire URL from your browser's address bar\n\n\
            5️⃣ Come back here and paste the URL\n\n\
            📋 The URL will look like:\nhttps://auth.tesla.com/void/callback?code=...\n\n\
            ⏳ Waiting for your authentication URL...",
            auth_url
        ),
    )
    .disable_web_page_preview(true)
    .await?;

    // Update dialogue state with code_verifier
    dialogue
        .update(State::WaitingForAuthCode {
            code_verifier: pkce_params.code_verifier,
        })
        .await?;

    log::info!(
        "LOGIN: Sent auth URL to user {} and updated dialogue state",
        chat_id
    );
    Ok(())
}

async fn receive_auth_url(
    bot: Bot,
    dialogue: TeslaDialogue,
    msg: Message,
    code_verifier: String,
) -> Result<()> {
    if let Some(text) = msg.text() {
        // Check if this is a Tesla callback URL
        if text.contains("auth.tesla.com/void/callback") {
            return receive_auth_url_internal(bot, dialogue, msg.chat.id, text, code_verifier)
                .await;
        } else {
            bot.send_message(
                msg.chat.id,
                "That doesn't look like a Tesla authentication URL. \n\n\
                Please copy the entire URL from your browser after logging in. \n\
                It should start with: https://auth.tesla.com/void/callback?code=...",
            )
            .await?;
        }
    }

    Ok(())
}

async fn order_status(bot: Bot, msg: Message) -> Result<()> {
    let chat_id = msg.chat.id;

    // Get authentication from database
    let mut conn = notifine::establish_connection();
    let auth = match tesla_auth::table
        .filter(tesla_auth::chat_id.eq(chat_id.0))
        .first::<TeslaAuth>(&mut conn)
        .optional()?
    {
        Some(auth) => auth,
        None => {
            bot.send_message(
                chat_id,
                "You need to login first! Use /login to authenticate.",
            )
            .await?;
            return Ok(());
        }
    };

    let client = Client::new();
    let crypto = get_token_crypto()?;

    // Decrypt the access token
    let mut access_token = crypto.decrypt(&auth.access_token)?;

    // Check if token is valid, refresh if needed
    let token_valid = is_token_valid(&access_token).unwrap_or(false);
    if !token_valid {
        // Decrypt refresh token for use
        let refresh_token = crypto.decrypt(&auth.refresh_token)?;
        match refresh_tokens(&client, &refresh_token).await {
            Ok(new_tokens) => {
                // Encrypt new tokens before updating database
                let encrypted_access_token = crypto.encrypt(&new_tokens.access_token)?;

                // Update tokens in database
                diesel::update(tesla_auth::table.filter(tesla_auth::chat_id.eq(chat_id.0)))
                    .set((
                        tesla_auth::access_token.eq(&encrypted_access_token),
                        tesla_auth::expires_in.eq(new_tokens.expires_in as i64),
                        tesla_auth::updated_at.eq(diesel::dsl::now),
                    ))
                    .execute(&mut conn)?;

                access_token = new_tokens.access_token;
            }
            Err(e) => {
                // Delete the invalid auth and ask user to login again
                diesel::delete(tesla_auth::table.filter(tesla_auth::chat_id.eq(chat_id.0)))
                    .execute(&mut conn)?;

                bot.send_message(
                    chat_id,
                    format!("Token expired or invalid: {}. Your authentication has been cleared. Please /login again.", e),
                )
                .await?;
                return Ok(());
            }
        }
    }

    // Fetch orders
    bot.send_message(chat_id, "Fetching your Tesla orders... 🔄")
        .await?;

    match retrieve_orders(&client, &access_token).await {
        Ok(orders) => {
            if orders.is_empty() {
                bot.send_message(chat_id, "No orders found on your Tesla account.")
                    .await?;
            } else {
                // Load existing order data from database
                let existing_order = tesla_orders::table
                    .filter(tesla_orders::chat_id.eq(chat_id.0))
                    .first::<TeslaOrder>(&mut conn)
                    .optional()?;

                let mut old_snapshots: Vec<OrderSnapshot> = Vec::new();
                if let Some(existing) = &existing_order {
                    if let Ok(stored_data) =
                        serde_json::from_value::<Vec<OrderSnapshot>>(existing.order_data.clone())
                    {
                        old_snapshots = stored_data;
                    }
                }

                // Create new snapshots from Tesla API
                let mut new_snapshots = Vec::new();
                let mut all_changes = Vec::new();

                for order in &orders {
                    if let Ok(details) =
                        get_order_details(&client, &order.reference_number, &access_token).await
                    {
                        let new_snapshot = create_order_snapshot(order, &details);

                        // Compare with old snapshot if exists
                        if let Some(old_snapshot) = old_snapshots
                            .iter()
                            .find(|s| s.order_id == new_snapshot.order_id)
                        {
                            let changes = compare_orders(old_snapshot, &new_snapshot);
                            if !changes.is_empty() {
                                all_changes.push((new_snapshot.order_id.clone(), changes));
                            }
                        }

                        new_snapshots.push(new_snapshot);
                    }
                }

                // Save new snapshots to database
                let order_data = serde_json::to_value(&new_snapshots)?;
                if let Some(existing) = existing_order {
                    diesel::update(tesla_orders::table.filter(tesla_orders::id.eq(existing.id)))
                        .set((
                            tesla_orders::order_data.eq(&order_data),
                            tesla_orders::updated_at.eq(diesel::dsl::now),
                        ))
                        .execute(&mut conn)?;
                } else {
                    let new_order = NewTeslaOrder {
                        chat_id: chat_id.0,
                        order_data,
                    };
                    diesel::insert_into(tesla_orders::table)
                        .values(&new_order)
                        .execute(&mut conn)?;
                }

                // Show current order status using snapshots (REAL Tesla data only)
                for snapshot in new_snapshots.iter() {
                    let mut message = String::new();

                    // Add changes at the top if there were any for this order
                    if let Some((_, changes)) = all_changes
                        .iter()
                        .find(|(order_id, _)| order_id == &snapshot.order_id)
                    {
                        if !changes.is_empty() {
                            message = format_order_changes(&snapshot.order_id, changes, snapshot);
                            bot.send_message(chat_id, message).await?;
                            continue; // Skip the regular order details since they're already included
                        }
                    }

                    // Order Details
                    message.push_str("📋 Order Details:\n");
                    message.push_str(&format!("- Order ID: {}\n", snapshot.order_id));
                    message.push_str(&format!("- Status: {}\n", snapshot.status));
                    message.push_str(&format!("- Model: {}\n", snapshot.model));
                    message.push_str(&format!(
                        "- VIN: {}\n\n",
                        snapshot.vin.as_deref().unwrap_or("N/A")
                    ));

                    // Reservation Details
                    if snapshot.reservation_date.is_some() || snapshot.order_booked_date.is_some() {
                        message.push_str("📅 Reservation Details:\n");
                        if let Some(res_date) = &snapshot.reservation_date {
                            message.push_str(&format!(
                                "- Reservation Date: {}\n",
                                format_date(res_date)
                            ));
                        }
                        if let Some(book_date) = &snapshot.order_booked_date {
                            message.push_str(&format!(
                                "- Order Booked Date: {}\n",
                                format_date(book_date)
                            ));
                        }
                        message.push('\n');
                    }

                    // Vehicle Status
                    if snapshot.vehicle_odometer.is_some() {
                        message.push_str("🚙 Vehicle Status:\n");
                        if let Some(odometer) = snapshot.vehicle_odometer {
                            let odo_type = snapshot.odometer_type.as_deref().unwrap_or("KM");
                            message.push_str(&format!(
                                "- Vehicle Odometer: {:.2} {}\n",
                                odometer, odo_type
                            ));
                        }
                        message.push('\n');
                    }

                    // Delivery Information
                    message.push_str("🚚 Delivery Information:\n");
                    if let Some(routing) = snapshot.routing_location {
                        message.push_str(&format!("- Routing Location: {}\n", routing));
                    }
                    if let Some(window) = &snapshot.delivery_window {
                        message.push_str(&format!("- Delivery Window: {}\n", window));
                    }
                    if let Some(eta) = &snapshot.eta_to_delivery_center {
                        message
                            .push_str(&format!("- ETA to Delivery Center: {}\n", format_date(eta)));
                    }
                    if let Some(appointment) = &snapshot.delivery_appointment {
                        message.push_str(&format!(
                            "- Delivery Appointment: {}\n",
                            format_date(appointment)
                        ));
                    } else {
                        message.push_str("- Delivery Appointment: N/A\n");
                    }

                    bot.send_message(chat_id, message).await?;
                }
            }
        }
        Err(e) => {
            bot.send_message(chat_id, format!("Failed to fetch orders: {}", e))
                .await?;
        }
    }

    Ok(())
}

async fn logout(bot: Bot, msg: Message) -> Result<()> {
    let chat_id = msg.chat.id;
    log::info!("LOGOUT: User {} initiated logout", chat_id);

    // Delete authentication from database
    let mut conn = notifine::establish_connection();
    let deleted = diesel::delete(tesla_auth::table.filter(tesla_auth::chat_id.eq(chat_id.0)))
        .execute(&mut conn)?;

    if deleted > 0 {
        log::info!("LOGOUT: User {} successfully logged out", chat_id);
        bot.send_message(
            chat_id,
            "Successfully logged out! Your authentication has been removed.",
        )
        .await?;
    } else {
        log::info!("LOGOUT: User {} was not logged in", chat_id);
        bot.send_message(chat_id, "You are not logged in.").await?;
    }

    Ok(())
}

async fn enable_monitoring(bot: Bot, msg: Message) -> Result<()> {
    let chat_id = msg.chat.id;
    let mut conn = notifine::establish_connection();

    // Check if user is authenticated
    let auth_exists = tesla_auth::table
        .filter(tesla_auth::chat_id.eq(chat_id.0))
        .first::<TeslaAuth>(&mut conn)
        .optional()?;

    match auth_exists {
        Some(_) => {
            // Enable monitoring
            diesel::update(tesla_auth::table.filter(tesla_auth::chat_id.eq(chat_id.0)))
                .set(tesla_auth::monitoring_enabled.eq(true))
                .execute(&mut conn)?;

            bot.send_message(
                chat_id,
                "✅ Automatic monitoring enabled!\n\nI'll check your Tesla order status every 5 minutes and notify you of any changes.",
            )
            .await?;

            log::info!("Monitoring enabled for chat {}", chat_id);
        }
        None => {
            bot.send_message(
                chat_id,
                "You need to login first! Use /login to authenticate.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn disable_monitoring(bot: Bot, msg: Message) -> Result<()> {
    let chat_id = msg.chat.id;
    let mut conn = notifine::establish_connection();

    // Check if user is authenticated
    let auth_exists = tesla_auth::table
        .filter(tesla_auth::chat_id.eq(chat_id.0))
        .first::<TeslaAuth>(&mut conn)
        .optional()?;

    match auth_exists {
        Some(_) => {
            // Disable monitoring
            diesel::update(tesla_auth::table.filter(tesla_auth::chat_id.eq(chat_id.0)))
                .set(tesla_auth::monitoring_enabled.eq(false))
                .execute(&mut conn)?;

            bot.send_message(
                chat_id,
                "🔕 Automatic monitoring disabled.\n\nYou can still check your order status manually with /orderstatus",
            )
            .await?;

            log::info!("Monitoring disabled for chat {}", chat_id);
        }
        None => {
            bot.send_message(
                chat_id,
                "You need to login first! Use /login to authenticate.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn monitoring_status(bot: Bot, msg: Message) -> Result<()> {
    let chat_id = msg.chat.id;
    let mut conn = notifine::establish_connection();

    // Check if user is authenticated and get monitoring status
    let auth = tesla_auth::table
        .filter(tesla_auth::chat_id.eq(chat_id.0))
        .first::<TeslaAuth>(&mut conn)
        .optional()?;

    match auth {
        Some(auth_data) => {
            let status_emoji = if auth_data.monitoring_enabled {
                "✅"
            } else {
                "🔕"
            };
            let status_text = if auth_data.monitoring_enabled {
                "enabled"
            } else {
                "disabled"
            };

            let message = format!(
                "{} Automatic monitoring is currently {}\n\n\
                📊 Monitoring Details:\n\
                • Check interval: Every 5 minutes\n\
                • Notifications: Only when changes are detected\n\
                • Last check: Check logs for details\n\n\
                Commands:\n\
                • Enable: /enablemonitoring\n\
                • Disable: /disablemonitoring",
                status_emoji,
                status_text.to_uppercase()
            );

            bot.send_message(chat_id, message).await?;
        }
        None => {
            bot.send_message(
                chat_id,
                "You need to login first! Use /login to authenticate.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_callback(bot: Bot, q: CallbackQuery) -> Result<()> {
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

async fn fetch_and_display_orders(
    bot: &Bot,
    chat_id: ChatId,
    client: &Client,
    access_token: &str,
) -> Result<()> {
    let _conn = notifine::establish_connection();
    match retrieve_orders(client, access_token).await {
        Ok(orders) => {
            log::info!(
                "ORDER_FETCH: Retrieved {} orders for user {}",
                orders.len(),
                chat_id
            );
            if orders.is_empty() {
                log::info!("ORDER_FETCH: No orders found for user {}", chat_id);
                bot.send_message(chat_id, "No orders found on your Tesla account.")
                    .await?;
            } else {
                // Format and send detailed order information (no database storage)
                for (i, order) in orders.iter().enumerate() {
                    if let Ok(details) =
                        get_order_details(client, &order.reference_number, access_token).await
                    {
                        let mut message = String::new();

                        // Header
                        message.push_str(&format!("🚗 Tesla Order {}\n", i + 1));
                        message.push_str(&format!("{}\n\n", "=".repeat(45)));

                        // Order Details
                        message.push_str("📋 Order Details:\n");
                        message.push_str(&format!("- Order ID: {}\n", order.reference_number));
                        message.push_str(&format!("- Status: {}\n", order.order_status));
                        message.push_str(&format!("- Model: {}\n", order.model_code));
                        message.push_str(&format!(
                            "- VIN: {}\n\n",
                            order.vin.as_deref().unwrap_or("N/A")
                        ));

                        // Extract detailed information from tasks
                        if let Some(tasks) = details.get("tasks") {
                            // Reservation Details
                            if let Some(registration) = tasks.get("registration") {
                                if let Some(order_details) = registration.get("orderDetails") {
                                    message.push_str("📅 Reservation Details:\n");

                                    if let Some(reservation_date) = order_details
                                        .get("reservationDate")
                                        .and_then(|v| v.as_str())
                                    {
                                        message.push_str(&format!(
                                            "- Reservation Date: {}\n",
                                            format_date(reservation_date)
                                        ));
                                    }

                                    if let Some(order_booked_date) = order_details
                                        .get("orderBookedDate")
                                        .and_then(|v| v.as_str())
                                    {
                                        message.push_str(&format!(
                                            "- Order Booked Date: {}\n",
                                            format_date(order_booked_date)
                                        ));
                                    }

                                    message.push('\n');

                                    // Vehicle Status
                                    message.push_str("🚙 Vehicle Status:\n");

                                    if let Some(odometer) = order_details
                                        .get("vehicleOdometer")
                                        .and_then(|v| v.as_f64())
                                    {
                                        let odometer_type = order_details
                                            .get("vehicleOdometerType")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("KM");
                                        message.push_str(&format!(
                                            "- Vehicle Odometer: {:.2} {}\n",
                                            odometer, odometer_type
                                        ));
                                    }

                                    message.push('\n');

                                    // Delivery Information
                                    message.push_str("🚚 Delivery Information:\n");

                                    if let Some(routing_location) = order_details
                                        .get("vehicleRoutingLocation")
                                        .and_then(|v| v.as_u64())
                                    {
                                        message.push_str(&format!(
                                            "- Routing Location: {}\n",
                                            routing_location
                                        ));
                                    }
                                }
                            }

                            // Scheduling information
                            if let Some(scheduling) = tasks.get("scheduling") {
                                if let Some(delivery_window) = scheduling
                                    .get("deliveryWindowDisplay")
                                    .and_then(|v| v.as_str())
                                {
                                    message.push_str(&format!(
                                        "- Delivery Window: {}\n",
                                        delivery_window
                                    ));
                                }

                                if let Some(appointment) = scheduling
                                    .get("apptDateTimeAddressStr")
                                    .and_then(|v| v.as_str())
                                {
                                    message.push_str(&format!(
                                        "- Delivery Appointment: {}\n",
                                        format_date(appointment)
                                    ));
                                } else {
                                    message.push_str("- Delivery Appointment: N/A\n");
                                }
                            }

                            // Final Payment / ETA information
                            if let Some(final_payment) = tasks.get("finalPayment") {
                                if let Some(data) = final_payment.get("data") {
                                    if let Some(eta) =
                                        data.get("etaToDeliveryCenter").and_then(|v| v.as_str())
                                    {
                                        message.push_str(&format!(
                                            "- ETA to Delivery Center: {}\n",
                                            format_date(eta)
                                        ));
                                    }
                                }
                            }
                        }

                        message.push_str(&format!("\n{}\n", "=".repeat(45)));

                        bot.send_message(chat_id, message).await?;
                    }
                }
            }
        }
        Err(e) => {
            bot.send_message(
                chat_id,
                format!("Successfully logged in! ✅\nFailed to fetch orders: {}. You can try again with /orderstatus", e),
            )
            .await?;
        }
    }
    Ok(())
}

async fn receive_auth_url_internal(
    bot: Bot,
    dialogue: TeslaDialogue,
    chat_id: ChatId,
    url_text: &str,
    code_verifier: String,
) -> Result<()> {
    // Parse the URL
    let parsed_url = match Url::parse(url_text) {
        Ok(url) => url,
        Err(_) => {
            bot.send_message(
                chat_id,
                "Invalid URL format. Please send the complete URL starting with https://",
            )
            .await?;
            return Ok(());
        }
    };

    // Extract authorization code
    let query_pairs: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

    if let Some(error) = query_pairs.get("error") {
        bot.send_message(chat_id, format!("Authentication error: {}", error))
            .await?;
        dialogue.update(State::Start).await?;
        return Ok(());
    }

    if let Some(code) = query_pairs.get("code") {
        // Exchange code for tokens
        let client = Client::new();
        match exchange_code_for_tokens(&client, code, &code_verifier).await {
            Ok(tokens) => {
                // Encrypt tokens before saving to database
                let crypto = get_token_crypto()?;
                let encrypted_access_token = crypto.encrypt(&tokens.access_token)?;
                let encrypted_refresh_token = crypto.encrypt(&tokens.refresh_token)?;

                // Save encrypted tokens to database
                let mut conn = notifine::establish_connection();
                let new_auth = NewTeslaAuth {
                    chat_id: chat_id.0,
                    access_token: &encrypted_access_token,
                    refresh_token: &encrypted_refresh_token,
                    expires_in: tokens.expires_in as i64,
                    token_type: &tokens.token_type,
                };

                diesel::insert_into(tesla_auth::table)
                    .values(&new_auth)
                    .execute(&mut conn)?;

                bot.send_message(
                    chat_id,
                    "Successfully logged in! ✅\nFetching your Tesla orders...",
                )
                .await?;

                // Automatically fetch and display order status
                fetch_and_display_orders(&bot, chat_id, &client, &tokens.access_token).await?;

                dialogue.update(State::Start).await?;
            }
            Err(e) => {
                bot.send_message(chat_id, format!("Failed to authenticate: {}", e))
                    .await?;
                dialogue.update(State::Start).await?;
            }
        }
    } else {
        bot.send_message(chat_id, "Authorization code not found in the URL. Please make sure you copied the complete URL.")
            .await?;
    }

    Ok(())
}

async fn teslacron(bot: Bot, msg: Message) -> Result<()> {
    use std::env;

    let chat_id = msg.chat.id;

    // Check if the user is admin
    let admin_chat_id: i64 = match env::var("TELEGRAM_ADMIN_CHAT_ID") {
        Ok(id) => match id.parse::<i64>() {
            Ok(parsed_id) => parsed_id,
            Err(_) => {
                bot.send_message(
                    chat_id,
                    "❌ Error: Invalid TELEGRAM_ADMIN_CHAT_ID configuration",
                )
                .await?;
                return Ok(());
            }
        },
        Err(_) => {
            bot.send_message(chat_id, "❌ Error: TELEGRAM_ADMIN_CHAT_ID not configured")
                .await?;
            return Ok(());
        }
    };

    if msg.chat.id.0 != admin_chat_id {
        bot.send_message(
            chat_id,
            "Sorry, this command is only available to administrators.",
        )
        .await?;
        return Ok(());
    }

    let interval_text = msg
        .text()
        .and_then(|text| text.split_once(' ').map(|(_, interval)| interval.trim()));

    match interval_text {
        Some(interval_str) if !interval_str.is_empty() => {
            match interval_str.parse::<u64>() {
                Ok(seconds) => {
                    // Set minimum interval to 1 second, maximum to 24 hours (86400 seconds)
                    let clamped_seconds = seconds.clamp(1, 86400);

                    // Import and use the function from tesla_monitor
                    crate::services::tesla_monitor::set_tesla_monitoring_interval(clamped_seconds);

                    let current_interval =
                        crate::services::tesla_monitor::get_tesla_monitoring_interval();

                    bot.send_message(
                        chat_id,
                        format!(
                            "✅ Tesla monitoring interval updated!\n\
                            New interval: {} seconds\n\
                            Note: Changes will take effect on the next monitoring cycle.",
                            current_interval
                        ),
                    )
                    .await?;
                }
                Err(_) => {
                    bot.send_message(
                        chat_id,
                        "❌ Invalid interval. Please provide a valid number of seconds.\n\
                        Usage: /teslacron <seconds>\n\
                        Example: /teslacron 10 (for 10 seconds)\n\
                        Minimum: 1 second, Maximum: 86400 seconds (24 hours)",
                    )
                    .await?;
                }
            }
        }
        _ => {
            let current_interval = crate::services::tesla_monitor::get_tesla_monitoring_interval();
            bot.send_message(
                chat_id,
                format!(
                    "📊 Tesla Monitoring Configuration\n\
                    Current interval: {} seconds\n\n\
                    To change the interval, use:\n\
                    /teslacron <seconds>\n\n\
                    Examples:\n\
                    • /teslacron 10 (10 seconds)\n\
                    • /teslacron 300 (5 minutes)\n\
                    • /teslacron 3600 (1 hour)\n\n\
                    Minimum: 1 second, Maximum: 86400 seconds (24 hours)",
                    current_interval
                ),
            )
            .await?;
        }
    }

    Ok(())
}

pub async fn run_tesla_bot(bot: Bot) {
    let handler = schema();
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![InMemStorage::<State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_snapshot_creation() {
        let snapshot = OrderSnapshot {
            order_id: "test123".to_string(),
            status: "PRODUCTION".to_string(),
            model: "MODEL_3".to_string(),
            vin: None,
            reservation_date: None,
            order_booked_date: None,
            vehicle_odometer: Some(100.0),
            odometer_type: Some("KM".to_string()),
            routing_location: Some(9999),
            delivery_window: Some("Feb 1-8, 2024".to_string()),
            eta_to_delivery_center: None,
            delivery_appointment: None,
        };

        assert_eq!(snapshot.vehicle_odometer, Some(100.0));
        assert_eq!(snapshot.routing_location, Some(9999));
        assert_eq!(snapshot.delivery_window, Some("Feb 1-8, 2024".to_string()));
    }

    #[test]
    fn test_format_date() {
        // Test with valid ISO date string
        let iso_date = "2025-05-28T16:06:16.884647";
        let formatted = format_date(iso_date);
        assert_eq!(formatted, "28 May 2025 16:06:16");

        // Test with another valid ISO date string
        let iso_date2 = "2025-06-30T00:00:00";
        let formatted2 = format_date(iso_date2);
        assert_eq!(formatted2, "30 June 2025 00:00:00");

        // Test with invalid date string - should return original
        let invalid_date = "invalid-date";
        let formatted_invalid = format_date(invalid_date);
        assert_eq!(formatted_invalid, "invalid-date");
    }
}

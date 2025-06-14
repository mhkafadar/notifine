use anyhow::Result;
use diesel::prelude::*;
use log::{error, info};
use notifine::{
    crypto::TokenCrypto,
    models::{TeslaAuth, TeslaOrder},
    schema::{tesla_auth, tesla_orders},
};
use reqwest::Client;
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::*;
use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::bots::tesla_bot::{
    compare_orders, create_order_snapshot, format_order_summary, get_order_details,
    refresh_access_token_if_needed, retrieve_orders, OrderSnapshot,
};
use notifine::i18n::I18N;

const BATCH_SIZE: usize = 5;
const DEFAULT_CHECK_INTERVAL_SECS: u64 = 10; // 5 minutes

// Global atomic variable to store the current check interval in seconds
static CHECK_INTERVAL_SECS: AtomicU64 = AtomicU64::new(DEFAULT_CHECK_INTERVAL_SECS);

pub fn set_tesla_monitoring_interval(seconds: u64) {
    CHECK_INTERVAL_SECS.store(seconds, Ordering::Relaxed);
    info!("Tesla monitoring interval updated to {} seconds", seconds);
}

pub fn get_tesla_monitoring_interval() -> u64 {
    CHECK_INTERVAL_SECS.load(Ordering::Relaxed)
}

pub async fn start_tesla_monitoring() -> Result<()> {
    info!("Starting Tesla order monitoring service");

    loop {
        let current_interval_secs = get_tesla_monitoring_interval();
        let check_interval_duration = Duration::from_secs(current_interval_secs);

        info!(
            "Tesla monitoring running with interval: {} seconds",
            current_interval_secs
        );

        match check_all_tesla_orders().await {
            Ok(_) => info!("Tesla order check completed successfully"),
            Err(e) => error!("Error during Tesla order check: {}", e),
        }

        // Sleep for the current interval
        sleep(check_interval_duration).await;
    }
}

async fn check_all_tesla_orders() -> Result<()> {
    let mut conn = notifine::establish_connection();
    let semaphore = Arc::new(Semaphore::new(BATCH_SIZE));
    let client = Arc::new(Client::new());
    let token = env::var("TESLA_TELOXIDE_TOKEN")?;
    let bot = Arc::new(Bot::new(token));

    // Get all authenticated users with monitoring enabled
    let auth_users = tesla_auth::table
        .filter(tesla_auth::monitoring_enabled.eq(true))
        .load::<TeslaAuth>(&mut conn)?;

    info!(
        "Checking {} Tesla accounts for order updates",
        auth_users.len()
    );

    for auth in auth_users {
        let permit = semaphore.clone().acquire_owned().await?;
        let client = client.clone();
        let bot = bot.clone();
        let chat_id = auth.chat_id;

        tokio::spawn(async move {
            if let Err(e) = check_user_orders(auth, client, bot).await {
                error!("Error checking orders for chat {}: {}", chat_id, e);
            }
            drop(permit);
        });
    }

    // Wait a bit to ensure all tasks complete
    sleep(Duration::from_secs(10)).await;
    Ok(())
}

async fn check_user_orders(auth: TeslaAuth, client: Arc<Client>, bot: Arc<Bot>) -> Result<()> {
    let chat_id = ChatId(auth.chat_id);
    let crypto = get_token_crypto()?;

    let access_token = match refresh_access_token_if_needed(&client, &auth, &crypto).await {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to refresh token for chat {}: {}", auth.chat_id, e);

            info!(
                "Clearing invalid tokens for chat {} to force re-authentication",
                auth.chat_id
            );
            let mut conn = notifine::establish_connection();
            if let Err(delete_err) =
                diesel::delete(tesla_auth::table.filter(tesla_auth::chat_id.eq(auth.chat_id)))
                    .execute(&mut conn)
            {
                error!(
                    "Failed to clear invalid tokens for chat {}: {}",
                    auth.chat_id, delete_err
                );
            } else if let Err(notify_err) = bot.send_message(
                chat_id,
                "🔐 Your Tesla authentication has expired and been cleared. Please use /login to re-authenticate and continue receiving order updates."
            ).await {
                error!("Failed to notify user {} about token expiration: {}", auth.chat_id, notify_err);
            }

            return Ok(());
        }
    };

    let orders = match retrieve_orders(&client, &access_token).await {
        Ok(orders) => orders,
        Err(e) => {
            error!("Failed to retrieve orders for chat {}: {}", auth.chat_id, e);
            return Ok(());
        }
    };

    let mut conn = notifine::establish_connection();

    for order in orders {
        // Get detailed order information
        let details = match get_order_details(&client, &order.reference_number, &access_token).await
        {
            Ok(details) => details,
            Err(e) => {
                error!(
                    "Failed to get order details for {}: {}",
                    order.reference_number, e
                );
                continue;
            }
        };

        // Create snapshot
        let new_snapshot = create_order_snapshot(&order, &details);

        // Check for existing order data
        let existing_order = tesla_orders::table
            .filter(tesla_orders::chat_id.eq(auth.chat_id))
            .first::<TeslaOrder>(&mut conn)
            .optional()?;

        if let Some(existing) = existing_order {
            // Parse existing snapshots (should be Vec<OrderSnapshot>)
            if let Ok(old_snapshots) =
                serde_json::from_value::<Vec<OrderSnapshot>>(existing.order_data.clone())
            {
                // Find the matching old snapshot for this order
                if let Some(old_snapshot) = old_snapshots
                    .iter()
                    .find(|s| s.order_id == new_snapshot.order_id)
                {
                    // Check for changes
                    let changes = compare_orders(old_snapshot, &new_snapshot);

                    if !changes.is_empty() {
                        // Get user language preference
                        let user_language = I18N.get_user_language(auth.chat_id);

                        // Send notification about changes using the same formatting as /orderstatus
                        let message =
                            format_order_summary(&new_snapshot, Some(&changes), &user_language);
                        bot.send_message(chat_id, message).await?;
                        info!("Sent update notification to chat {}", auth.chat_id);
                    }
                }

                // Update the stored snapshots - replace the matching order or add if new
                let mut updated_snapshots = old_snapshots.clone();
                if let Some(existing_idx) = updated_snapshots
                    .iter()
                    .position(|s| s.order_id == new_snapshot.order_id)
                {
                    updated_snapshots[existing_idx] = new_snapshot;
                } else {
                    updated_snapshots.push(new_snapshot);
                }

                diesel::update(tesla_orders::table.filter(tesla_orders::chat_id.eq(auth.chat_id)))
                    .set((
                        tesla_orders::order_data.eq(serde_json::to_value(&updated_snapshots)?),
                        tesla_orders::updated_at.eq(diesel::dsl::now),
                    ))
                    .execute(&mut conn)?;
            }
        } else {
            // First time checking - store the snapshot without notification
            let initial_snapshots = vec![new_snapshot];
            diesel::insert_into(tesla_orders::table)
                .values(notifine::models::NewTeslaOrder {
                    chat_id: auth.chat_id,
                    order_data: serde_json::to_value(&initial_snapshots)?,
                })
                .execute(&mut conn)?;

            info!("Stored initial order snapshot for chat {}", auth.chat_id);
        }
    }

    Ok(())
}

fn get_token_crypto() -> Result<TokenCrypto> {
    let key = env::var("TESLA_ENCRYPTION_KEY")
        .map_err(|_| anyhow::anyhow!("TESLA_ENCRYPTION_KEY not set"))?;
    TokenCrypto::new(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tesla_monitoring_interval() {
        // Test default interval
        assert_eq!(get_tesla_monitoring_interval(), DEFAULT_CHECK_INTERVAL_SECS);

        // Test setting interval
        set_tesla_monitoring_interval(10);
        assert_eq!(get_tesla_monitoring_interval(), 10);

        // Test setting another interval
        set_tesla_monitoring_interval(3600);
        assert_eq!(get_tesla_monitoring_interval(), 3600);

        // Reset to default
        set_tesla_monitoring_interval(DEFAULT_CHECK_INTERVAL_SECS);
        assert_eq!(get_tesla_monitoring_interval(), DEFAULT_CHECK_INTERVAL_SECS);
    }
}

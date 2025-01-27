use serde::de::DeserializeOwned;
use ureq::serde_json;
use serde_urlencoded;

pub fn parse_webhook_payload<T>(body: &[u8]) -> Result<T, String>
where
    T: DeserializeOwned,
{
    // First try to parse as regular JSON
    if let Ok(event) = serde_json::from_slice(body) {
        return Ok(event);
    }

    // If that fails, try to parse as URL-encoded form data
    let body_str = String::from_utf8_lossy(body);
    if !body_str.starts_with("payload=") {
        return Err("Invalid format: not JSON and not form data".to_string());
    }

    // Extract the payload value
    let form_data: std::collections::HashMap<String, String> = 
        serde_urlencoded::from_str(&body_str)
            .map_err(|e| format!("Failed to parse form data: {}", e))?;
    
    let payload = form_data.get("payload")
        .ok_or_else(|| "No payload field found".to_string())?;

    // Parse the JSON payload
    serde_json::from_str(payload)
        .map_err(|e| format!("Failed to parse JSON payload: {}", e))
} 
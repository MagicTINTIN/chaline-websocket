use serde_json::Value;
use tracing::info;
use std::fs;

pub fn load_configs() -> Option<Vec<String>> {
    // read file (return None on error)
    let json_data = fs::read_to_string("configs.json").ok()?;
    // parse JSON (return None on error)
    let v: Value = serde_json::from_str(&json_data).ok()?;

    // optional: log the name if present
    if let Some(name) = v.get("name").and_then(|x| x.as_str()) {
        info!("Loading configuration '{}'...", name);
    } else {
        info!("Loading configuration (name missing)...");
    }
    
    // extract rooms as Vec<String> (return None if "rooms" not an array)
    let rooms = v["rooms"]
        .as_array()?                         // Option<&Vec<Value>>
        .iter()
        .filter_map(|r| r.as_str().map(|s| s.to_string()))
        .collect::<Vec<String>>();

    Some(rooms)
}

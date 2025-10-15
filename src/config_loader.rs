use serde_json::Value;
use std::{fmt, fs};
use tracing::{error, info};

pub enum RoomKind {
    Broadcast,
    Group(String),
    Individual(String),
}

impl fmt::Display for RoomKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoomKind::Broadcast => write!(f, "Broadcast"),
            RoomKind::Group(url) => write!(f, "Group->({})", url),
            RoomKind::Individual(url) => write!(f, "Individual->({})", url),
        }
    }
}

pub struct RoomConfig {
    pub prefix: String,
    pub kind: RoomKind,
    pub authorized_messages: Vec<String>,
}

pub fn load_configs() -> Option<Vec<String>> {
    // ? = return None on error
    let json_data = fs::read_to_string("configs.json").ok()?;
    let v: Value = serde_json::from_str(&json_data).ok()?;

    if let Some(name) = v.get("name").and_then(|x| x.as_str()) {
        info!("Loading global configuration '{}'...", name);
    } else {
        info!("Loading global configuration (name missing)...");
    }

    let rooms = v["rooms"]
        .as_array()?
        .iter()
        .filter_map(|r| r.as_str().map(|s| s.to_string()))
        .collect::<Vec<String>>();

    Some(rooms)
}

pub fn load_room_config(path: &String) -> Option<RoomConfig> {
    let json_data = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&json_data).ok()?;

    if let Some(name) = v.get("name").and_then(|x| x.as_str()) {
        info!("Loading room configuration '{}'...", name);
    } else {
        info!("Loading room configuration (name missing)...");
    }

    let prefix = v.get("prefix").and_then(|x| x.as_str()).unwrap_or_else(|| {
        error!("prefix field not found, please define it!");
        "none"
    });
    let kind = v.get("type").and_then(|x| x.as_str());

    let auth_msgs = v["authorized"]
        .as_array().unwrap_or(&vec![])
        .iter()
        .filter_map(|r| r.as_str().map(|s| s.to_string()))
        .collect::<Vec<String>>();

    match kind {
        Some("broadcast") => Some(RoomConfig {
            prefix: prefix.to_string(),
            kind: RoomKind::Broadcast,
            authorized_messages:auth_msgs,
        }),
        Some("group") => {
            if let Some(url) = v.get("fetchURL").and_then(|x| x.as_str()) {
                Some(RoomConfig {
                    prefix: prefix.to_string(),
                    kind: RoomKind::Group(String::from(url)),
                    authorized_messages:auth_msgs,
                })
            } else {
                error!("missing fetchURL field necessary for 'group' and 'individual' room types!");
                None
            }
        }
        Some("individual") => {
            if let Some(url) = v.get("fetchURL").and_then(|x| x.as_str()) {
                Some(RoomConfig {
                    prefix: prefix.to_string(),
                    kind: RoomKind::Individual(String::from(url)),
                    authorized_messages:auth_msgs,
                })
            } else {
                error!("missing fetchURL field necessary for 'group' and 'individual' room types!");
                None
            }
        }
        Some(k) => {
            error!("room type '{}' unknown", k);
            None
        }
        None => {
            error!("type field not found, please define it!");
            None
        }
    }
}

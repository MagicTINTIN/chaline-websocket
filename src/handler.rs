use std::collections::HashMap;

use crate::config_loader::RoomConfig;

pub struct SplittedMessage {
    // prefix: String,
    content: String,
    room_name: String,
    id: Option<String>,
}

fn split_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<SplittedMessage> {
    let parts = msg.splitn(3, ":").collect::<Vec<_>>();

    if parts.len() < 2 {
        return None;
    }

    if !confs.contains_key(parts[0]) {
        return None;
    }

    if parts.len() == 2 {
        Some(SplittedMessage{content: parts[1].to_string(), room_name: parts[0].to_string(), id: None})
    } else {
        Some(SplittedMessage{content: parts[1].to_string(), room_name: parts[0].to_string(), id: Some(parts[2].to_string())})
    }
}

pub struct WebSocketAction {

}

pub fn handle_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<WebSocketAction> {
    None
}
use std::collections::HashMap;

use crate::config_loader::RoomConfig;

pub struct Message {
    // prefix: String,
    content: String,
    room: RoomConfig,
    id: Option<String>,
}

fn split_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<Message> {
    let parts = msg.split(":").collect::<Vec<_>>();

    if parts.len() < 2 {
        return None;
    }

    if !confs.contains_key(parts[0]) {
        return None;
    }

    None
}

pub struct WebSocketAction {

}

pub fn handle_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<WebSocketAction> {
    None
}
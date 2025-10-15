use std::collections::HashMap;

use crate::{
    com::{str_to_roomgroup, SplittedMessage},
    config_loader::RoomConfig,
};

fn split_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<SplittedMessage> {
    let parts = msg.splitn(2, ":").collect::<Vec<_>>();

    if parts.len() < 2 {
        return None;
    }

    if !confs.contains_key(parts[0]) {
        return None;
    }

    if let Some(rg) = str_to_roomgroup(confs, parts[0].to_string()) {
        Some(SplittedMessage {
            content: parts[1].to_string(),
            room_group: rg,
        })
    } else {
        None
    }
}

pub struct WebSocketAction {}

pub fn handle_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<WebSocketAction> {
    None
}

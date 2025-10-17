use std::collections::HashMap;

use tracing::warn;

use crate::{
    com::{str_to_roomgroup, RoomGroup, SplittedMessage},
    config_loader::RoomConfig,
};

fn split_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<SplittedMessage> {
    let parts = msg.splitn(2, ":").collect::<Vec<_>>();

    println!("split> {:?}", parts);

    if parts.len() < 2 {
        return None;
    }

    // if !confs.contains_key(parts[0]) {
    //     return None;
    // }

    if let Some(rg) = str_to_roomgroup(confs, parts[0]) {
        if confs.contains_key(&rg.room) {
            Some(SplittedMessage {
                content: parts[1].trim().to_string(),
                room_group: rg,
            })
        } else {
            warn!("{} room not found", rg.room);
            None
        }
    } else {
        None
    }
}

fn is_authorized_message(msg: String, conf: &RoomConfig) -> bool {
    conf.authorized_messages.is_empty() || conf.authorized_messages.contains(&msg)
}

pub struct WebSocketAction {
    pub send_message: String,
    pub room_group: RoomGroup,
    pub room_config: RoomConfig,
}

pub fn handle_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<WebSocketAction> {
    let splitted_msg = split_message(msg, confs)?;
    let conf = confs.get(&splitted_msg.room_group.room)?;

    if !is_authorized_message(splitted_msg.content.clone(), conf) {
        warn!(
            "Unauthorized messaage '{}', authorized {:?}",
            splitted_msg.content, conf.authorized_messages
        );
        return None;
    }

    Some(WebSocketAction {
        send_message: splitted_msg.content,
        room_group: splitted_msg.room_group,
        room_config: conf.clone(),
    })
}

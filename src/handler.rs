use std::collections::HashMap;

use tracing::warn;

use crate::{
    com::{
        disconnect_group, does_room_group_exists, str_to_roomgroup, RoomGroup, ServerMap, SharedM,
        SplittedMessage,
    },
    config_loader::RoomConfig,
};

fn split_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<SplittedMessage> {
    let parts = msg.splitn(2, ":").collect::<Vec<_>>();

    // println!("split> {:?}", parts);

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
    (conf.authorized_messages.is_empty() && conf.message_map.is_empty())
        || conf.authorized_messages.contains(&msg)
        || conf.message_map.contains_key(&msg)
}

pub async fn handle_group_destruction(
    room_group_name: String,
    confs: &HashMap<String, RoomConfig>,
    smap: &SharedM<ServerMap>,
) {
    // let parts = room_group_name.splitn(2, ":").collect::<Vec<_>>();

    // // println!("split> {:?}", parts);

    // if parts.len() < 2 {
    //     return ;
    // }

    // // if !confs.contains_key(parts[0]) {
    // //     return None;
    // // }

    if let Some(rg) = str_to_roomgroup(confs, &room_group_name) {
        if confs.contains_key(&rg.room) && rg.group.is_some() && rg.fetch_url.is_some() {
            match does_room_group_exists(&rg.fetch_url.unwrap(), &rg.group.unwrap()).await {
                Ok(v) => {
                    if !v {
                        disconnect_group(smap, &rg.full_roomgroup).await;
                    }
                }
                Err(_) => {
                    disconnect_group(smap, &rg.full_roomgroup).await;
                }
            }
        } else {
            warn!("{} room/group not found", rg.room);
        }
    }
}

pub struct WebSocketAction {
    pub send_message: String,
    pub room_group: RoomGroup,
    pub room_config: RoomConfig,
}

pub fn handle_message(msg: String, confs: &HashMap<String, RoomConfig>) -> Option<WebSocketAction> {
    // if msg.starts_with("-") {
    //     return None;
    // }

    let splitted_msg = split_message(msg, confs)?;
    let conf = confs.get(&splitted_msg.room_group.room)?;

    if !is_authorized_message(splitted_msg.content.clone(), conf) {
        warn!(
            "Unauthorized messaage '{}', authorized {:?}",
            splitted_msg.content, conf.authorized_messages
        );
        return None;
    }

    let msg_to_send = conf
        .message_map
        .get(&splitted_msg.content)
        .unwrap_or(&splitted_msg.content)
        .clone();

    Some(WebSocketAction {
        send_message: msg_to_send,
        room_group: splitted_msg.room_group,
        room_config: conf.clone(),
    })
}

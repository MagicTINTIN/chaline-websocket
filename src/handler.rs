use crate::{
    add_client_to_rg,
    com::{broadcast_to_group, ClientMap},
};
use std::collections::HashMap;

use tokio_tungstenite::tungstenite::Message;
use tracing::{trace, warn};

use crate::{
    com::{
        disconnect_group, does_room_group_exists, str_to_roomgroup, ClientRoom, RoomGroup,
        ServerMap, SharedM, SplittedMessage,
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
) -> bool {
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
                        true
                    } else {
                        false
                    }
                }
                Err(_) => {
                    disconnect_group(smap, &rg.full_roomgroup).await;
                    true
                }
            }
        } else {
            warn!("{} room/group not found", rg.room);
            false
        }
    } else {
        false
    }
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

pub async fn handle_raw_message(
    configs: &HashMap<String, RoomConfig>,
    rooms: &SharedM<ServerMap>,
    clients: &SharedM<ClientMap>,
    msg: Message,
    opt_client_r: Option<&ClientRoom>,
) -> bool {
    if let Message::Text(txt) = msg {
        trace!("Received: {}", txt);

        if txt.starts_with("-") {
            if handle_group_destruction(txt[1..txt.len()].to_string(), configs, &rooms).await {
                if let Some(client_r) = opt_client_r {
                    warn!(
                        "Closing connection {}: group is closing...",
                        client_r.global_id
                    );
                } else {
                    warn!(
                        "Closing connection by HTTP-Push: group is closing...",
                    );
                }
            } else {
                if let Some(client_r) = opt_client_r {
                    let _ = client_r.c.send(Message::Close(None));
                    warn!(
                        "Closing connection {}: client was trying to close wrong group...",
                        client_r.global_id
                    );
                }
            };

            // let _ = client_r.c.send(Message::Close(None)); //tx.
            return false;
        } else if let Some(res) = handle_message(txt.to_string(), &configs) {
            if let Some(client_r) = opt_client_r {
                if !add_client_to_rg(
                    &rooms,
                    &clients,
                    res.room_config,
                    res.room_group.clone(),
                    client_r.clone(),
                )
                .await
                {
                    let _ = client_r.c.send(Message::Close(None));
                    warn!(
                        "Closing connection {}: error while connecting to invalid group...",
                        client_r.global_id
                    );
                    return false;
                }
            }
            broadcast_to_group(&rooms, &res.room_group.full_roomgroup, res.send_message).await;
        } else {
            if let Some(client_r) = opt_client_r {
                warn!(
                    "Closing connection {}: unknown/invalid message",
                    client_r.global_id
                );
                let _ = client_r.c.send(Message::Close(None)); //tx.
            }
            return false;
        }

        // if txt.contains("new micasend message") {
        //     println!("Broadcasting ping");

        //     // Broadcast to all clients
        //     let clients_guard = clients.lock().unwrap();
        //     for client in clients_guard.iter() {
        //         let _ = client
        //             .c
        //             .send(Message::Text("new message notification".to_string().into()));
        //     }
        // }
    }
    true
}

use std::{collections::HashMap, sync::Arc};

use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::config_loader::{RoomConfig, RoomKind};
use tracing::{info, warn};

#[derive(Clone)]
pub struct ClientRoom {
    pub c: mpsc::UnboundedSender<Message>,
    pub global_id: u64,
    pub prefix: String,
}

#[derive(Clone)]
pub struct ServerRoom {
    pub clients: Vec<ClientRoom>,
    pub config: RoomConfig,
}

pub type ServerMap = HashMap<String, ServerRoom>;
pub type SharedServerMap = Arc<Mutex<ServerMap>>;

fn does_room_group_exists(url: &String) -> bool {
    if let Ok(response) = reqwest::blocking::get(url) {
        if response.status().is_success() {
            if let Ok(text) = response.text() {
                return text.trim().contains("yes");
            }
        }
    }
    false
}

pub async fn add_client(
    map: SharedServerMap,
    confs: &HashMap<String, RoomConfig>,
    room_group_name: String,
    client: ClientRoom,
) {
    let name_parts = room_group_name.split("/").collect::<Vec<_>>();

    if name_parts.len() > 2 {
        warn!("{} is not a valid room/group name", room_group_name);
        return;
    }
    if name_parts.len() < 2
        && !confs.contains_key(&room_group_name)
        && confs.get(&room_group_name).unwrap().kind != RoomKind::Broadcast
    {
        warn!(
            "{} is not a valid room name / not a broadcast room",
            room_group_name
        );
        return;
    }

    let conf = confs.get(name_parts[0]);
    if conf.is_none() {
        warn!("{} is not a valid room", room_group_name);
        return;
    }

    let mut guard = map.lock().await;
    if let Some(rg_name) = guard.get_mut(&room_group_name) {
        rg_name.clients.push(client.clone());
        info!(
            "New client ({}) added to {}",
            client.global_id, room_group_name
        );
    } else if does_room_group_exists(&room_group_name) {
        guard.insert(
            room_group_name.clone(),
            ServerRoom {
                clients: vec![client.clone()],
                config: conf.unwrap().clone(),
            },
        );
        info!(
            "Client ({}) added to {} (new group)",
            client.global_id, room_group_name
        );
    } else {
        warn!(
            "Client ({}) can't be added to {} (invalid group)",
            client.global_id, room_group_name
        );
    }
    // guard released at end of scope
}

pub async fn broadcast_to_group(map: SharedServerMap, group: &str, msg: Message) {
    // hold lock while collecting clients
    let clients = {
        let guard = map.lock().await;
        guard.get(group).cloned()
    };

    if let Some(clients) = clients {
        // now send without holding the lock
        for tx in clients.clients {
            // send consumes msg, so clone if necessary
            let _ = tx.c.send(msg.clone());
        }
    }
}

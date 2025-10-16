use std::{collections::HashMap, sync::Arc};

use tokio::sync::{mpsc, Mutex}; //{mpsc, Mutex}
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::config_loader::{self, RoomConfig, RoomKind};
use tracing::{info, warn};

#[derive(Clone)]
pub struct RoomGroup {
    pub full_roomgroup: String,
    pub room: String,
    pub group: Option<String>,
    pub fetchURL: Option<String>
}

pub struct SplittedMessage {
    // prefix: String,
    pub content: String,
    pub room_group: RoomGroup,
}

pub fn str_to_roomgroup(confs: &HashMap<String, RoomConfig>, str: String) -> Option<RoomGroup> {
    let name_parts = str.split("/").collect::<Vec<_>>();

    if name_parts.len() > 2 {
        warn!("{} is not a valid room/group name", str);
        return None;
    }
    if name_parts.len() < 2
        && !confs.contains_key(&str)
        && confs.get(&str).unwrap().kind != RoomKind::Broadcast
    {
        warn!("{} is not a valid room name / not a broadcast room", str);
        return None;
    }

    let conf = confs.get(name_parts[0]);
    if conf.is_none() {
        warn!("{} is not a valid room", str);
        return None;
    }

    match conf.unwrap().kind.clone() {
        RoomKind::Broadcast => Some(RoomGroup {
            full_roomgroup: name_parts[0].to_string(),
            room: name_parts[0].to_string(),
            group: None,
            fetchURL: None,
        }),
        RoomKind::Group(url)=> Some(RoomGroup {
            full_roomgroup: name_parts[0].to_string() + &"/".to_string() + &name_parts[1].to_string(),
            room: name_parts[0].to_string(),
            group: Some(name_parts[1].to_string()),
            fetchURL: Some(url)
        }),
        RoomKind::Individual(url)=> Some(RoomGroup {
            full_roomgroup: name_parts[0].to_string() + &"/".to_string() + &name_parts[1].to_string(),
            room: name_parts[0].to_string(),
            group: Some(name_parts[1].to_string()),
            fetchURL: Some(url)
        }),
    }
}

#[derive(Clone)]
pub struct ClientRoom {
    pub c: mpsc::UnboundedSender<Message>,
    pub global_id: u64,
    // pub prefix: String,
}

#[derive(Clone)]
pub struct ServerRoom {
    pub clients: Vec<ClientRoom>,
    pub config: RoomConfig,
}

pub type ServerMap = HashMap<String, ServerRoom>;
pub type ClientMap = HashMap<u64, Vec<String>>;
pub type SharedM<T> = Arc<Mutex<T>>;

fn does_room_group_exists(url: &String, group: &String) -> bool {
    if let Ok(response) = reqwest::blocking::get(url.to_owned() + group) {
        if response.status().is_success() {
            if let Ok(text) = response.text() {
                return text.trim().contains("yes");
            }
        }
    }
    false
}

pub async fn add_client(
    map: SharedM<ServerMap>,
    // confs: &HashMap<String, RoomConfig>,
    conf: RoomConfig,
    rg: RoomGroup,
    client: ClientRoom,
) {
    let mut guard = map.lock().await;
    if let Some(rg_name) = guard.get_mut(&rg.full_roomgroup) {
        rg_name.clients.push(client.clone());
        info!(
            "New client ({}) added to {}",
            client.global_id, rg.full_roomgroup
        );
    } else if conf.kind == config_loader::RoomKind::Broadcast || does_room_group_exists(
        &rg.fetchURL.unwrap(),
        &rg.group.unwrap(),
    ) {
        guard.insert(
            rg.full_roomgroup.clone(),
            ServerRoom {
                clients: vec![client.clone()],
                config: conf,
            },
        );
        info!(
            "Client ({}) added to {} (new group)",
            client.global_id, &rg.full_roomgroup
        );
    } else {
        warn!(
            "Client ({}) can't be added to {} (invalid group)",
            client.global_id, &rg.full_roomgroup
        );
    }
    // guard released at end of scope
}

pub fn rm_client (map: SharedM<ServerMap>, id: u64) {
    
}

pub async fn broadcast_to_group(map: SharedM<ServerMap>, group: &str, msg: Message) {
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
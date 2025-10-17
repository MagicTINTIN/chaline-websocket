use std::{
    collections::HashMap,
    sync::Arc,
};

use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::config_loader::{self, RoomConfig, RoomKind};
use tracing::{info, warn};

#[derive(Clone)]
pub struct RoomGroup {
    pub full_roomgroup: String,
    pub room: String,
    pub group: Option<String>,
    pub fetch_url: Option<String>,
}

pub struct SplittedMessage {
    // prefix: String,
    pub content: String,
    pub room_group: RoomGroup,
}

pub fn str_to_roomgroup(confs: &HashMap<String, RoomConfig>, name: &str) -> Option<RoomGroup> {
    let name_parts = name.split('/').collect::<Vec<_>>();

    println!("str2rg> {:?}", name_parts);

    if name_parts.len() > 2 {
        warn!("{} is not a valid room/group name", name);
        return None;
    }

    if name_parts.len() == 1 {
        match confs.get(name) {
            Some(conf) if conf.kind == RoomKind::Broadcast => {
                return Some(RoomGroup {
                    full_roomgroup: name.to_string(),
                    room: name.to_string(),
                    group: None,
                    fetch_url: None,
                });
            }
            Some(_) => {
                warn!("{} is not a broadcast room", name);
                return None;
            }
            None => {
                warn!("{} is not a valid room name", name);
                return None;
            }
        }
    }

    // else len == 2
    let room = name_parts[0];
    let group = name_parts[1];

    let conf = match confs.get(room) {
        Some(c) => c,
        None => {
            warn!("{} is not a valid room", name);
            return None;
        }
    };

    match conf.kind.clone() {
        RoomKind::Broadcast => {
            warn!(
                "{} is a broadcast room and does not accept a group suffix",
                room
            );
            None
        }
        RoomKind::Group(url) | RoomKind::Individual(url) => Some(RoomGroup {
            full_roomgroup: format!("{}/{}", room, group),
            room: room.to_string(),
            group: Some(group.to_string()),
            fetch_url: Some(url),
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
    // pub config: RoomConfig,
}

pub type ServerMap = HashMap<String, ServerRoom>;
pub type ClientMap = HashMap<u64, Vec<String>>;
pub type SharedM<T> = Arc<Mutex<T>>;

pub async fn does_room_group_exists(url: &str, group: &str) -> Result<bool, reqwest::Error> {
    let full_url = format!("{url}{group}");
    let resp = reqwest::get(&full_url).await?;
    if resp.status().is_success() {
        let text = resp.text().await?;
        Ok(text.trim().contains("yes"))
    } else {
        Ok(false)
    }
}

pub async fn add_client_to_rg(
    smap: &SharedM<ServerMap>,
    cmap: &SharedM<ClientMap>,
    // confs: &HashMap<String, RoomConfig>,
    conf: RoomConfig,
    rg: RoomGroup,
    client: ClientRoom,
) {
    {
        let mut guard = cmap.lock().await;
        if let Some(cmap_client) = guard.get_mut(&client.global_id) {
            if cmap_client.contains(&rg.full_roomgroup) {
                return;
            } else {
                cmap_client.push(rg.full_roomgroup.clone());
            }
        } else {
            guard.insert(client.global_id, vec![]);
        }
    }
    let mut guard = smap.lock().await;
    if let Some(rg_name) = guard.get_mut(&rg.full_roomgroup) {
        rg_name.clients.push(client.clone());
        info!(
            "New client ({}) added to {}",
            client.global_id, rg.full_roomgroup
        );
        return;
    }

    let is_valid = if conf.kind == config_loader::RoomKind::Broadcast {
        info!("broadcast room, no need to check group");
        true
    } else {
        match (&rg.fetch_url, &rg.group) {
            (Some(url), Some(group)) => match does_room_group_exists(&url, &group).await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(
                            "Error checking remote group existence for {} ({}): {:?}",
                            url, group, e
                        );
                        false
                    }
                },
            _ => false, // this case shouldn't appear
        }
    };

    if is_valid {
        guard.insert(
            rg.full_roomgroup.clone(),
            ServerRoom {
                clients: vec![client.clone()],
                // config: conf,
            },
        );
        info!(
            "Client ({}) added to {} (new group)",
            client.global_id, &rg.full_roomgroup
        );
        // guard released at end of scope
    } else {
        warn!(
            "Client ({}) can't be added to {} (invalid group)",
            client.global_id, &rg.full_roomgroup
        );
    }
}

pub async fn rm_client(smap: &SharedM<ServerMap>, cmap: &SharedM<ClientMap>, id: u64) {
    {
        let mut guard = cmap.lock().await;
        guard.remove(&id);
    }
    {
        let mut guard = smap.lock().await;
        // for each room, remove clients with `global_id == id`, then remove empty rooms.
        guard.retain(|_room_name, server_room| {
            server_room.clients.retain(|c| c.global_id != id);
            !server_room.clients.is_empty()
        });
    }
}

pub async fn broadcast_to_group(smap: &SharedM<ServerMap>, group: &str, msg: String) {
    // hold lock while collecting clients
    let maybe_roomgroup = {
        let guard = smap.lock().await;
        guard.get(group).cloned()
    };

    if let Some(roomgroup) = maybe_roomgroup {
        // now send without holding the lock
        for client in roomgroup.clients {
            // send consumes msg, so clone if necessary
            let _ = client.c.send(msg.clone().into());
        }
    }
}

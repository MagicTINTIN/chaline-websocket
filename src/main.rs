use anyhow::Context;
use com::{add_client_to_rg, rm_client, ClientMap, ClientRoom, ServerMap, SharedM};
use config_loader::RoomConfig;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio_rustls::rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::accept_async;
use tracing::{error, info};

use crate::handler::handle_raw_message;

mod com;
mod config_loader;
mod handler;
mod http_push;

static GLOBAL_COUNTER: AtomicU64 = AtomicU64::new(0);

fn get_new_client_id() -> u64 {
    // atomic
    GLOBAL_COUNTER.fetch_add(1, Ordering::Relaxed)
}

// fn get_global_counter() -> u64 {
//     // load provides atomic read. No `unsafe` needed.
//     GLOBAL_COUNTER.load(Ordering::SeqCst)
// }

static ROOM_CONFIGS: OnceLock<HashMap<String, RoomConfig>> = OnceLock::new();

fn get_rooms_config() -> &'static HashMap<String, RoomConfig> {
    ROOM_CONFIGS.get_or_init(|| -> HashMap<String, RoomConfig> {
        let mut m = HashMap::new();
        let habile = config_loader::load_configs().unwrap_or(vec![]);
        for e in habile.into_iter() {
            let rc = config_loader::load_room_config(&e).unwrap();

            info!(
                "> {}\n[{}]={}: {} messages authorized",
                &e,
                &rc.prefix,
                &rc.kind,
                &rc.authorized_messages.len().max(rc.message_map.len())
            );

            if !rc.message_map.is_empty() {
                info!("{:?}", rc.message_map);
            }
            // mut_conf_vec.push(rc);
            m.insert(rc.prefix.clone(), rc);
        }
        m
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ssl_disabled = args.contains(&"--no-ssl".to_string());
    tracing::subscriber::set_global_default(tracing_subscriber::fmt::Subscriber::new()).unwrap();

    if ssl_disabled {
        main_without_tls().unwrap();
    } else {
        main_tls().unwrap();
    }
}

#[tokio::main]
async fn main_without_tls() -> anyhow::Result<()> {
    // shared list of clients
    let clients: SharedM<ClientMap> = Arc::new(Mutex::new(HashMap::new()));
    let rooms: SharedM<ServerMap> = Arc::new(Mutex::new(HashMap::new()));

    // HTTP PUSH listener
    let http_rooms = rooms.clone();
    let http_clients = clients.clone();
    tokio::spawn(async move {
        if let Err(e) =
            http_push::spawn_simple_push_listener(http_rooms, http_clients, "127.0.0.1:6442").await
        {
            tracing::error!("push listener failed: {:?}", e);
        }
    });

    // TCP listener
    let listener = TcpListener::bind("[::]:8080").await?;
    println!("Listening on ws://[::]:8080");

    while let Ok((stream, _)) = listener.accept().await {
        let clients = Arc::clone(&clients);
        let rooms = Arc::clone(&rooms);

        tokio::spawn(async move {
            // upgrade to WebSocket
            let ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(err) => {
                    error!("WebSocket handshake failed: {}", err);
                    return;
                }
            };
            let client_id = get_new_client_id();
            println!("New WebSocket connection ({}) established", client_id);

            let configs = get_rooms_config();

            // Split the WebSocket stream into read and write halves
            let (mut write, mut read) = ws_stream.split();

            // add this client to the shared list
            let (tx, mut rx) = mpsc::unbounded_channel();
            let client_r = ClientRoom {
                c: tx,
                global_id: client_id,
            };

            {
                let mut guard = clients.lock().await;
                guard.insert(client_id, vec![]);
            }

            // sending messages to the client
            let send_task = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if write.send(msg).await.is_err() {
                        break; // Client disconnected
                    }
                }
            });

            // receiving messages from the client
            while let Some(Ok(msg)) = read.next().await {
                if !handle_raw_message(configs, &rooms, &clients, msg, Some(&client_r)).await {
                    break;
                };
            }

            info!("Socket connection ended");

            // remove the client from the shared list
            rm_client(&rooms, &clients, client_id).await;

            // wait for the send task to finish
            let _ = send_task.await;

            drop(client_r.c);
        });
    }

    Ok(())
}

#[tokio::main]
async fn main_tls() -> anyhow::Result<()> {
    // Works only for one certificate
    let cert =
        CertificateDer::from_pem_file("/etc/ssl/private/mtc").context("no certificate found")?;
    let key = PrivateKeyDer::from_pem_file("/etc/ssl/private/mtk").context("no key found")?;

    // TLS server
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;
    let acceptor = TlsAcceptor::from(Arc::new(config));

    // shared list of clients
    let clients: SharedM<ClientMap> = Arc::new(Mutex::new(HashMap::new())); //tokio::sync::
    let rooms: SharedM<ServerMap> = Arc::new(Mutex::new(HashMap::new())); //tokio::sync::
                                                                          // let clients = Arc::new(Mutex::new(Vec::new()));

    // HTTP PUSH listener
    let http_rooms = rooms.clone();
    let http_clients = clients.clone();
    tokio::spawn(async move {
        if let Err(e) =
            http_push::spawn_simple_push_listener(http_rooms, http_clients, "127.0.0.1:6442").await
        {
            tracing::error!("push listener failed: {:?}", e);
        }
    });

    // TCP listener
    let listener = TcpListener::bind("[::]:8443").await?;
    println!("Listening on wss://[::]:8443");

    while let Ok((stream, _)) = listener.accept().await {
        let acceptor = acceptor.clone();
        let clients = Arc::clone(&clients);
        let rooms = Arc::clone(&rooms);
        // let clients = Arc::clone(&clients);

        tokio::spawn(async move {
            // accept TLS connection
            let tls_stream = match acceptor.accept(stream).await {
                Ok(tls_stream) => tls_stream,
                Err(err) => {
                    error!("TLS handshake failed: {}", err);
                    return;
                }
            };

            // upgrade to WebSocket
            let ws_stream = match accept_async(tls_stream).await {
                Ok(ws) => ws,
                Err(err) => {
                    error!("WebSocket handshake failed: {}", err);
                    return;
                }
            };
            let client_id = get_new_client_id();
            println!("New WebSocket connection ({}) established", client_id);

            let configs = get_rooms_config();

            // Split the WebSocket stream into read and write halves
            let (mut write, mut read) = ws_stream.split();

            // add this client to the shared list
            let (tx, mut rx) = mpsc::unbounded_channel();
            let client_r = ClientRoom {
                c: tx,
                global_id: client_id,
            };

            {
                let mut guard = clients.lock().await;
                guard.insert(client_id, vec![]);
            }

            // sending messages to the client
            let send_task = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if write.send(msg).await.is_err() {
                        break; // Client disconnected
                    }
                }
            });

            // receiving messages from the client
            while let Some(Ok(msg)) = read.next().await {
                if !handle_raw_message(configs, &rooms, &clients, msg, Some(&client_r)).await {
                    break;
                };
            }

            info!("Socket connection ended");

            // remove the client from the shared list
            rm_client(&rooms, &clients, client_id).await;

            // wait for the send task to finish
            let _ = send_task.await;

            // release socket file
            drop(client_r.c);
        });
    }

    Ok(())
}

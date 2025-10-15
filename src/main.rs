use anyhow::Context;
use config_loader::RoomConfig;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{error, info, trace};

mod config_loader;

static ROOM_CONFIGS: OnceLock<HashMap<String, RoomConfig>> = OnceLock::new();

fn get_rooms() -> &'static HashMap<String, RoomConfig> {
    ROOM_CONFIGS.get_or_init(|| -> HashMap<String, RoomConfig> {
        let mut m = HashMap::new();
        let habile = config_loader::load_configs().unwrap_or(vec![]);
        for i in 0..habile.len() {
            println!("> {}", habile[i]);
            let rc = config_loader::load_room_config(&habile[i]).unwrap();

            println!(
                "[{}]={}: {} messages authorized",
                &rc.prefix,
                &rc.kind,
                &rc.authorized_messages.len()
            );
            // mut_conf_vec.push(rc);
            m.insert(rc.prefix.clone(), rc);
        }
        m
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let ssl_disabled = args.contains(&"--no-ssl".to_string());
    if ssl_disabled {
        todo!("Version without SSL not implemented yet!");
    }

    tracing::subscriber::set_global_default(tracing_subscriber::fmt::Subscriber::new()).unwrap();
    // for i in 0..habile.len() {
    //     println!("> {}", habile[i]);
    //     let rc = config_loader::load_room_config(&habile[i]).unwrap();
    //     // mut_conf_vec.push(rc);
    //     mut_conf_hmap.insert(rc.prefix.clone(), rc);
    // }

    // let conf_vec: Arc<Vec<RoomConfig>> = Arc::new(mut_conf_vec);
    // let conf_vec: Arc<[RoomConfig]> = Arc::from(mut_conf_vec.into_boxed_slice());

    // let s = conf_vec.clone();
    // for i in 0..conf_vec.len() {
    //     println!("[{}]={}: {} messages authorized", &conf_vec[i].prefix, &conf_vec[i].kind, &conf_vec[i].authorized_messages.len());
    // }
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
    let clients = Arc::new(Mutex::new(Vec::new()));

    // TCP listener
    let listener = TcpListener::bind("[::]:8443").await?;
    println!("Listening on wss://[::]:8443");

    while let Ok((stream, _)) = listener.accept().await {
        let acceptor = acceptor.clone();
        let clients = Arc::clone(&clients);

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
            println!("New WebSocket connection established");

            let _room_map = get_rooms();

            // Split the WebSocket stream into read and write halves
            let (mut write, mut read) = ws_stream.split();

            // add this client to the shared list
            let (tx, mut rx) = mpsc::unbounded_channel();
            {
                let mut clients_guard = clients.lock().unwrap();
                clients_guard.push(tx);
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
                if let Message::Text(txt) = msg {
                    trace!("Received: {}", txt);
                    if txt.contains("new micasend message") {
                        println!("Broadcasting ping");

                        // Broadcast to all clients
                        let clients_guard = clients.lock().unwrap();
                        for client in clients_guard.iter() {
                            let _ = client
                                .send(Message::Text("new message notification".to_string().into()));
                        }
                    }
                }
            }

            info!("Socket connection ended");

            // remove the client from the shared list
            {
                let mut clients_guard = clients.lock().unwrap();
                clients_guard.retain(|client| !client.is_closed());
            }

            // wait for the send task to finish
            let _ = send_task.await;
        });
    }

    Ok(())
}

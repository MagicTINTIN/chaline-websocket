use anyhow::Context;
use futures::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConfig,
};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::accept_async;
use tracing::{error, info, trace};

mod config_loader;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing::subscriber::set_global_default(tracing_fmt::FmtSubscriber::new()).unwrap();
    let habile = config_loader::load_configs();
    debug!(habile);
    let args: Vec<String> = std::env::args().collect();
    let ssl_disabled = args.contains(&"--no-ssl".to_string());
    if !ssl_disabled {}
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
                if let tokio_tungstenite::tungstenite::protocol::Message::Text(txt) = msg {
                    trace!("Received: {}", txt);
                    if txt.contains("new micasend message") {
                        println!("Broadcasting ping");

                        // Broadcast to all clients
                        let clients_guard = clients.lock().unwrap();
                        for client in clients_guard.iter() {
                            let _ = client.send(
                                tokio_tungstenite::tungstenite::protocol::Message::Text(
                                    "new message notification".to_string().into(),
                                ),
                            );
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

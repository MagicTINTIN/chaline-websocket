use futures::{SinkExt, StreamExt};
use std::env;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::accept_async;
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let ssl_disabled = args.contains(&"--no-ssl".to_string());
    if !ssl_disabled {}
    let cert_file = &mut BufReader::new(File::open("/etc/ssl/private/mtc")?);
    let key_file = &mut BufReader::new(File::open("/etc/ssl/private/mtk")?);
    let cert_chain = certs(cert_file)?
        .into_iter()
        .map(Certificate)
        .collect();
    let mut keys = pkcs8_private_keys(key_file)?;
    if keys.is_empty() {
        return Err("No private key found".into());
    }

    // TLS server
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, PrivateKey(keys.remove(0)))?;
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
                    eprintln!("TLS handshake failed: {}", err);
                    return;
                }
            };

            // upgrade to WebSocket
            let ws_stream = match accept_async(tls_stream).await {
                Ok(ws) => ws,
                Err(err) => {
                    eprintln!("WebSocket handshake failed: {}", err);
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
                    println!("Received: {}", txt);
                    if txt.contains("new micasend message") {
                        println!("Broadcasting ping");

                        // Broadcast to all clients
                        let clients_guard = clients.lock().unwrap();
                        for client in clients_guard.iter() {
                            let _ = client.send(tokio_tungstenite::tungstenite::protocol::Message::Text(
                                "new message notification".to_string(),
                            ));
                        }
                    }
                }
            }

            println!("Socket connection ended");

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


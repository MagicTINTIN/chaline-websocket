use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tracing::warn;
// use std::sync::Arc;
// use tokio::sync::Mutex;

// reuse your types
use crate::com::{ClientMap, ServerMap, SharedM};
use crate::get_rooms_config;
use crate::handler::handle_raw_message;

/// Spawn a tiny HTTP POST /push/<room> listener that reads raw body as message.
/// Bind only to 127.0.0.1 if you don't want it exposed externally.
pub async fn spawn_simple_push_listener(
    rooms: SharedM<ServerMap>,
    clients: SharedM<ClientMap>,
    bind: &str,
) -> anyhow::Result<()> {

    let configs = get_rooms_config();

    let listener = TcpListener::bind(bind).await?;
    tracing::info!("Push HTTP listener running on http://{}", bind);

    loop {
        let (mut stream, _peer) = listener.accept().await?;
        let rooms = rooms.clone();
        let clients = clients.clone();

        tokio::spawn(async move {
            // read until headers end "\r\n\r\n"
            let mut buf = [0u8; 4096];
            let mut req = Vec::<u8>::new();

            // read header bytes (simple loop)
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) => {
                        // connection closed
                        return;
                    }
                    Ok(n) => {
                        req.extend_from_slice(&buf[..n]);
                        if req.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                        // prevent extremely large headers
                        if req.len() > 64 * 1024 {
                            warn!("heavy http push request");
                            let _ = stream
                                .write_all(b"HTTP/1.1 413 Payload Too Large\r\n\r\n")
                                .await;
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }

            // split headers / partial body
            let header_end = req.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
            let headers = &req[..header_end];
            let mut body = req[header_end..].to_vec();

            // parse request line and headers (very small parser)
            let headers_str = match std::str::from_utf8(headers) {
                Ok(s) => s,
                Err(_) => {
                    let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
                    return;
                }
            };

            let mut lines = headers_str.split("\r\n");
            let request_line = lines.next().unwrap_or("");
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("");

            // only accept POST
            if method != "POST" {
                let _ = stream
                    .write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n")
                    .await;
                return;
            }

            // expect path like /push/<room>
            let prefix = "/push";
            if !path.starts_with(prefix) {
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").await;
                return;
            }
            // let encoded_room = &path[prefix.len()..];
            // let room = match percent_decode(encoded_room) {
            //     Ok(s) => s,
            //     Err(_) => {
            //         let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
            //         return;
            //     }
            // };

            // find Content-Length header
            let mut content_length: usize = 0;
            for line in lines {
                if line.is_empty() {
                    continue;
                }
                if let Some(v) = line.strip_prefix("Content-Length:") {
                    content_length = v.trim().parse::<usize>().unwrap_or(0);
                }
            }

            // read the rest of the body if needed
            while body.len() < content_length {
                match stream.read(&mut buf).await {
                    Ok(0) => break, // connection closed
                    Ok(n) => body.extend_from_slice(&buf[..n]),
                    Err(_) => {
                        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
                        return;
                    }
                }
            }

            // interpret body as utf-8 string (if non-UTF8 you can pass bytes)
            let message = match String::from_utf8(body) {
                Ok(s) => s,
                Err(_) => {
                    let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
                    return;
                }
            };

            handle_raw_message(configs, &rooms, &clients, Message::Text(message.into()), None).await;

            // reply OK
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await;
        });
    }
}

// tiny percent-decode helper for path segment (returns error on invalid hex)
fn _percent_decode(s: &str) -> Result<String, ()> {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err(());
                }
                let hi = bytes[i + 1];
                let lo = bytes[i + 2];
                let hex = [hi, lo];
                let hexstr = match std::str::from_utf8(&hex) {
                    Ok(h) => h,
                    Err(_) => return Err(()),
                };
                let val = u8::from_str_radix(hexstr, 16).map_err(|_| ())?;
                out.push(val as char);
                i += 3;
            }
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    Ok(out)
}

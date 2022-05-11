use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

mod packet;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let mut map: HashMap<String, TcpStream> = HashMap::new();

    let listener = TcpListener::bind("127.0.0.1:3001").await?;
    println!("Listening on TCP: 127.0.0.1:3001");

    loop {
        let (mut socket, _) = listener.accept().await?;
        println!("Accpeting new client...");
        let result = process_socket(&mut map, &mut socket).await?;
        if let Some(host) = result {
            map.insert(host, socket);
        }
    }
}

async fn process_socket(
    map: &mut HashMap<String, TcpStream>,
    socket: &mut TcpStream,
) -> std::io::Result<Option<String>> {
    let mut buffer = BytesMut::with_capacity(4096);
    let mut result = None;

    loop {
        let bytes_len = socket.read_buf(&mut buffer).await?;
        if bytes_len != 0 {
            let mut headers = [httparse::EMPTY_HEADER; 64];
            let mut req = httparse::Request::new(&mut headers);
            req.parse(&buffer).unwrap();

            let host_header = req.headers.iter().find(|h| h.name == "Host");

            if let Some(host) = host_header {
                let mut host = String::from_utf8_lossy(host.value).into_owned();

                if host.contains(':') {
                    let (domain, _port) = host.split_once(':').unwrap();
                    host = domain.to_string();
                }

                println!("Host: {host}");
                println!("map: {map:?}");
                // TODO: Tunnel to another socket.
                if let Some(tunnel_socket) = map.get_mut(&host) {
                    println!("writing message!");
                    tunnel_socket.write_all(&buffer).await?;
                    let mut response_buffer = BytesMut::with_capacity(4096);

                    let mut len = tunnel_socket.read_buf(&mut response_buffer).await?;
                    while len != 0 {
                        socket.write_all(&response_buffer).await?;
                        response_buffer.advance(len);
                        len = tunnel_socket.read_buf(&mut response_buffer).await?;
                    }

                    println!("receive and forward response!");
                } else {
                    socket
                        .write_all(b"HTTP/1.1 500 Internal Server Error\nContent-Length: 0\nConnection: Closed\n\n")
                        .await?;
                }
            } else {
                let packet = packet::Packet::parse(&buffer);
                println!("socket {socket:?}, receive: {packet:?}");

                if let packet::Packet::Init(resp) = packet {
                    socket.write_all(resp.as_bytes()).await?;
                    result = Some(resp);
                    break;
                }

                buffer.advance(bytes_len);
            }
        } else {
            break;
        }
    }

    Ok(result)
}

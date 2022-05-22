use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

mod packet;

type State = Arc<RwLock<HashMap<String, ControlChannel>>>;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let listener = TcpListener::bind("127.0.0.1:3001").await?;
    let state: State = Arc::new(RwLock::new(HashMap::new()));
    println!("Listening on TCP: 127.0.0.1:3001");
    loop {
        if let Ok((conn, _)) = listener.accept().await {
            println!("Accpeting new client...");
            let state = state.clone();
            tokio::spawn(async move {
                let _ = handle_connection(conn, state).await;
            });
        }
    }
}

async fn handle_connection(mut conn: TcpStream, state: State) -> std::io::Result<()> {
    let mut buffer = BytesMut::with_capacity(4096);

    let bytes_len = conn.read_buf(&mut buffer).await?;
    let packet = packet::Packet::parse(&buffer);
    match packet {
        packet::Packet::Init => {
            let domain = "test.rok.me".to_string();
            let success = packet::Packet::Success(domain.clone());
            conn.write_all(&bincode::serialize(&success).unwrap())
                .await?;
            println!("sent success msg");

            buffer.advance(bytes_len);
            let len = conn.read_buf(&mut buffer).await;
            if let packet::Packet::Ack = packet::Packet::parse(&buffer) {
                println!("receive ack from client");

                let mut state = state.write().await;

                // Ask client to create a data channel.
                // let create_data = &bincode::serialize(&packet::Packet::CreateData).unwrap();
                // conn.write_all(create_data).await?;
                // println!("sent create data msg");
                let cc = ControlChannel::new(conn);
                state.insert(domain, cc);
            }
        }
        packet::Packet::DataInit(domain) => {
            let mut state = state.write().await;

            if let Some(cc) = state.get_mut(&domain) {
                cc.send(conn);
            } else {
                println!("no control channel found for {domain}");
            }
        }
        _ => {
            println!("unexpected packet: {packet:?}");
        }
    }
    buffer.advance(bytes_len);

    Ok(())
}

#[derive(Debug)]
struct ControlChannel {
    conn: TcpStream,
}

impl ControlChannel {
    fn new(conn: TcpStream) -> Self {
        Self { conn }
    }

    fn send(&mut self, mut conn: TcpStream) {
        tokio::spawn(async move {
            let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
            println!("Listening to 127.0.0.1:3000");

            if let Ok((mut incoming, addr)) = listener.accept().await {
                println!("Accept incoming from {addr:?}");
                if conn
                    .write_all(&bincode::serialize(&packet::Packet::DataForward).unwrap())
                    .await
                    .is_ok()
                {
                    println!("copy bidirectional data: incoming, conn");
                    let _ = tokio::io::copy_bidirectional(&mut incoming, &mut conn).await;
                }
            }
        });
    }
}

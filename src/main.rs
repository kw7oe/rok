use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

mod packet;

type State = Arc<RwLock<HashMap<String, TcpStream>>>;

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
        packet::Packet::DataInit(_domain) => {
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
        }
        _ => {
            println!("unexpected packet: {packet:?}");
        }
    }
    buffer.advance(bytes_len);

    Ok(())
}

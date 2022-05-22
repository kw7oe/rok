use bytes::Buf;
use tokio::io::{AsyncReadExt, AsyncWriteExt as _};
use tokio::net::TcpStream;
mod packet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut tunnel_stream = TcpStream::connect("rok.me:3001").await?;
    let mut buf = bytes::BytesMut::with_capacity(1024);

    // Send a Init
    let init = packet::Packet::Init;
    tunnel_stream
        .write_all(&bincode::serialize(&init).unwrap())
        .await?;
    let len = tunnel_stream.read_buf(&mut buf).await;
    let domain = if let packet::Packet::Success(domain) = packet::Packet::parse(&buf) {
        println!("tunnel up!\nHost: {domain}");
        Some(domain)
    } else {
        None
    };

    if let Ok(len) = len {
        buf.advance(len);
    }

    if domain.is_none() {
        return Err("fail to init with server".into());
    }

    // Let tunnel know client is ready
    tunnel_stream
        .write_all(&bincode::serialize(&packet::Packet::Ack).unwrap())
        .await?;

    println!("control channel established!");

    let len = tunnel_stream.read_buf(&mut buf).await;
    if let packet::Packet::CreateData = packet::Packet::parse(&buf) {
        println!("receive command to create proxy");
        let handle = tokio::spawn(async move {
            if let Err(e) = run_data_channel(domain.unwrap()).await {
                println!("{:?}", e);
            }
        });

        let _ = handle.await;
        Ok(())
    } else {
        Err("invalid packet".into())
    }
}

async fn run_data_channel(domain: String) -> std::io::Result<()> {
    let mut conn = TcpStream::connect("rok.me:3001").await?;
    conn.write_all(&bincode::serialize(&packet::Packet::DataInit(domain)).unwrap())
        .await?;

    let packet = bincode::serialize(&packet::Packet::DataForward).unwrap();
    let mut buf = vec![0u8; packet.len()];
    conn.read_exact(&mut buf).await?;
    println!("buf: {buf:?}");

    if let packet::Packet::DataForward = packet::Packet::parse(&buf) {
        let mut local = TcpStream::connect("127.0.0.1:4000").await?;
        println!("copying data from {conn:?} to {local:?}");
        let (a, b) = tokio::io::copy_bidirectional(&mut conn, &mut local).await?;
        println!("{a}, {b}");
    } else {
        println!("no packets receive");
    }

    Ok(())
}

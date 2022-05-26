use bytes::Buf;
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt as _};
use tokio::net::TcpStream;
mod packet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt::init();
    let domain = init().await?;
    if let Err(e) = run_data_channel(domain).await {
        tracing::error!("{:?}", e);
    }

    Ok(())
}

async fn init() -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut cc = TcpStream::connect("rok.me:3001").await?;
    let mut buf = bytes::BytesMut::with_capacity(1024);

    // Send a Init
    let init = packet::Packet::Init;
    cc.write_all(&bincode::serialize(&init).unwrap()).await?;
    let len = cc.read_buf(&mut buf).await?;
    let domain = if let packet::Packet::Success(domain) = packet::Packet::parse(&buf) {
        tracing::info!("tunnel up!\nHost: {domain}");
        Some(domain)
    } else {
        None
    };
    buf.advance(len);

    if domain.is_none() {
        return Err("fail to init with server".into());
    }

    // Let tunnel know client is ready
    cc.write_all(&bincode::serialize(&packet::Packet::Ack).unwrap())
        .await?;

    tracing::trace!("control channel established!");

    tokio::spawn(async move {
        loop {
            let res = cc.read_exact(&mut [0u8; 1]).await;
            if let Err(err) = res {
                tracing::error!("receive error: {}", err);
                break;
            }
        }
    });

    domain.ok_or_else(|| "no domain return".into())
}

async fn run_data_channel(domain: String) -> std::io::Result<()> {
    loop {
        let mut conn = TcpStream::connect("rok.me:3001").await?;
        tracing::trace!("established data channel...");
        conn.write_all(&bincode::serialize(&packet::Packet::DataInit(domain.clone())).unwrap())
            .await?;

        let packet = bincode::serialize(&packet::Packet::DataForward).unwrap();
        let mut buf = vec![0u8; packet.len()];
        conn.read_exact(&mut buf).await?;

        if let packet::Packet::DataForward = packet::Packet::parse(&buf) {
            let mut local = TcpStream::connect("127.0.0.1:4000").await?;
            tracing::trace!("copy bidirectional data: conn, local");
            let _ = tokio::io::copy_bidirectional(&mut conn, &mut local).await?;
        } else {
            tracing::trace!("no packets receive");
        }
    }
}

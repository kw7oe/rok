use bytes::Buf;
use tokio::io::{AsyncReadExt, AsyncWriteExt as _};
use tokio::net::TcpStream;
mod packet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut cc = TcpStream::connect("rok.me:3001").await?;
    let mut buf = bytes::BytesMut::with_capacity(1024);

    // Send a Init
    let init = packet::Packet::Init;
    cc.write_all(&bincode::serialize(&init).unwrap()).await?;
    let len = cc.read_buf(&mut buf).await?;
    let domain = if let packet::Packet::Success(domain) = packet::Packet::parse(&buf) {
        println!("tunnel up!\nHost: {domain}");
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

    println!("control channel established!");

    let domain = domain.unwrap();
    loop {
        let len = cc.read_buf(&mut buf).await?;
        if let packet::Packet::CreateData = packet::Packet::parse(&buf) {
            println!("receive command to create proxy");
            let domain = domain.clone();
            tokio::spawn(async move {
                if let Err(e) = run_data_channel(domain).await {
                    println!("{:?}", e);
                }
            });
        } else {
            println!("receive invalid packet");
        }
        buf.advance(len);
    }
}

async fn run_data_channel(domain: String) -> std::io::Result<()> {
    let mut conn = TcpStream::connect("rok.me:3001").await?;
    conn.write_all(&bincode::serialize(&packet::Packet::DataInit(domain)).unwrap())
        .await?;

    let packet = bincode::serialize(&packet::Packet::DataForward).unwrap();
    let mut buf = vec![0u8; packet.len()];
    conn.read_exact(&mut buf).await?;

    if let packet::Packet::DataForward = packet::Packet::parse(&buf) {
        let mut local = TcpStream::connect("127.0.0.1:4000").await?;
        println!("copy bidirectional data: conn, local");
        let _ = tokio::io::copy_bidirectional(&mut conn, &mut local).await?;
    } else {
        println!("no packets receive");
    }

    Ok(())
}

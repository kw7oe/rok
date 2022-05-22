use tokio::io::{AsyncReadExt, AsyncWriteExt as _};
use tokio::net::TcpStream;
mod packet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let domain = "test.rok.me".to_string();
    if let Err(e) = run_data_channel(domain).await {
        println!("{:?}", e);
    }

    Ok(())
}

async fn run_data_channel(domain: String) -> std::io::Result<()> {
    loop {
        let mut conn = TcpStream::connect("rok.me:3001").await?;
        println!("established data channel...");
        conn.write_all(&bincode::serialize(&packet::Packet::DataInit(domain.clone())).unwrap())
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
    }
}

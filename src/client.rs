use bytes::Buf;
use tokio::io::{AsyncReadExt, AsyncWriteExt as _};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut target_stream = TcpStream::connect("localhost:4000").await?;
    let mut tunnel_stream = TcpStream::connect("rok.me:3001").await?;
    tunnel_stream.write_all(b"0").await?;

    let mut buf = bytes::BytesMut::with_capacity(4096);
    let len = tunnel_stream.read_buf(&mut buf).await?;
    println!("tunnel up!\nHost: {:?}", buf);
    buf.advance(len);

    loop {
        let mut target_buf = bytes::BytesMut::with_capacity(4096);

        // Copy bytes from tunnel to our targeted server:
        let len = tunnel_stream.read_buf(&mut buf).await?;
        target_stream.write_all(&buf).await?;
        buf.advance(len);

        // Copy response form targeted server back to tunnel:
        let mut len = target_stream.read_buf(&mut target_buf).await?;
        while len != 0 {
            tunnel_stream.write_all(&target_buf).await?;
            target_buf.advance(len);
            len = target_stream.read_buf(&mut target_buf).await?;
        }

        println!("receive request and send response...");
    }
}

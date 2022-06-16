use bytes::Buf;
use std::error::Error;
use std::io::{self, BufRead};
use std::pin::Pin;
use std::str;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt as _, ReadBuf};
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

    // Send heartbeat to server every 500 ms;
    tokio::spawn(async move {
        loop {
            let res = cc.write_all(&[1u8; 1]).await;
            if let Err(err) = res {
                tracing::error!("control channel is closed by remote peer {}", err);
                break;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
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

        // Two implementations:
        // 2 -- > reimplement copy_bidirectional with an event channel that backdoors
        //          A --------> B           send(Event::AtoB).await.expect("channel closed!");
        //          B --------> A           j

        if let packet::Packet::DataForward = packet::Packet::parse(&buf) {
            let mut local = TcpStream::connect("127.0.0.1:4000").await?;
            tracing::trace!("copy bidirectional data: conn, local");

            let mut logger = ConnectionLogger::new(Box::pin(conn), Box::pin(local));

            tokio::io::copy_bidirectional(&mut logger.dst, &mut logger.src).await;
            // starting from here
        }
    }
}

struct ConnectionLogger<'a, T: AsyncRead + AsyncWrite> {
    src: Logger<'a, T>,
    dst: Logger<'a, T>,
    counter: usize,
}

struct Logger<'a, T: AsyncRead + AsyncWrite> {
    // why do we need to box T? deref?
    inner: Pin<Box<T>>,
    manager: Option<&'a ConnectionLogger<'a, T>>,
}

impl<T: AsyncRead + AsyncWrite> ConnectionLogger<'_, T> {
    fn new(dst: Pin<Box<T>>, src: Pin<Box<T>>) -> Self {
        Self {
            dst: Logger {
                inner: dst,
                manager: None,
            },
            src: Logger {
                inner: src,
                manager: None,
            },
            counter: 0,
        }
    }
}

/// "implement the AsyncRead trait for the Logger struct where T is bound to AsyncRead"
impl<T: AsyncRead + AsyncWrite> AsyncRead for Logger<'_, T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>, // hint: that this method is supposed to be ran inside of a task buf: &mut ReadBuf<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let poll_result = self.inner.as_mut().poll_read(cx, buf);
        if poll_result.is_ready() {
            // do not log if the buffer is empty
            if buf.capacity() != buf.remaining() {
                let raw_http = str::from_utf8(buf.filled()).expect("failed to strify");
                let chunks: Vec<&str> = raw_http.split('\n').collect();
                if !chunks.is_empty() {
                    println!("{}", chunks[0]);
                    // println!("{}", self.packet_counter);
                }
            }
        }
        poll_result
    }
}

impl<T: AsyncWrite + AsyncRead> AsyncWrite for Logger<'_, T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.inner.as_mut().poll_write(cx, buf)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.inner.as_mut().poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.as_mut().poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.as_mut().poll_shutdown(cx)
    }
}

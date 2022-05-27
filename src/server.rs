use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock};

mod packet;

type State = Arc<RwLock<HashMap<String, ControlChannel>>>;
type DomainPort = (String, u16);

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let listener = TcpListener::bind("127.0.0.1:3001").await?;
    let domain_to_port = Arc::new(Mutex::new(vec![
        ("test.rok.me".to_string(), 3002),
        ("test2.rok.me".to_string(), 3003),
        ("test3.rok.me".to_string(), 3004),
    ]));

    let state: State = Arc::new(RwLock::new(HashMap::new()));
    tracing::info!("Listening on TCP: 127.0.0.1:3001");
    loop {
        if let Ok((conn, _)) = listener.accept().await {
            tracing::info!("Accpeting new client...");
            let state = state.clone();
            let domains = domain_to_port.clone();
            tokio::spawn(async move {
                let err = handle_connection(conn, domains, state).await;
            });
        }
    }
}

async fn handle_connection(
    mut conn: TcpStream,
    domains: Arc<Mutex<Vec<DomainPort>>>,
    state: State,
) -> std::io::Result<()> {
    let mut buffer = BytesMut::with_capacity(4096);

    let bytes_len = conn.read_buf(&mut buffer).await?;
    let packet = packet::Packet::parse(&buffer);
    match packet {
        packet::Packet::Init => {
            let mut domains_guard = domains.lock().await;
            let domain_port = domains_guard.pop();
            drop(domains_guard);

            if domain_port.is_none() {
                tracing::warn!("oops no more domain available, ignore you");
                return Ok(());
            }

            let domain_port = domain_port.unwrap();
            let domain = domain_port.0.clone();
            let success = packet::Packet::Success(domain.clone());
            conn.write_all(&bincode::serialize(&success).unwrap())
                .await?;
            tracing::trace!("sent success msg");

            buffer.advance(bytes_len);
            conn.read_buf(&mut buffer).await?;
            if let packet::Packet::Ack = packet::Packet::parse(&buffer) {
                tracing::trace!("receive ack from client");
                let mut state = state.write().await;
                let cc = ControlChannel::new(conn, domain_port, Arc::clone(&domains));
                state.insert(domain, cc);
            }
        }
        packet::Packet::DataInit(domain) => {
            let state = state.write().await;

            if let Some(cc) = state.get(&domain) {
                let _ = cc.data_tx.send(conn).await;
            }
        }
        _ => {
            println!("unexpected packet: {packet:?}");
        }
    }
    buffer.advance(bytes_len);

    Ok(())
}

struct ControlChannel {
    domain_port: DomainPort,
    data_tx: mpsc::Sender<TcpStream>,
}

impl ControlChannel {
    pub fn new(
        mut conn: TcpStream,
        domain_port: DomainPort,
        domains: Arc<Mutex<Vec<DomainPort>>>,
    ) -> Self {
        let (tx, mut rx): (_, mpsc::Receiver<TcpStream>) = mpsc::channel(32);

        // Clone so we could move into tokio async
        let domains = Arc::clone(&domains);
        let dp = domain_port.clone();
        tokio::spawn(async move {
            loop {
                let res = conn.read_exact(&mut [0u8; 1]).await;

                if let Err(err) = res {
                    tracing::error!("receive error: {}", err);
                    let mut domains_guard = domains.lock().await;
                    domains_guard.push(dp);
                    break;
                }
            }
        });

        let port = domain_port.1;
        tokio::spawn(async move {
            while let Some(mut conn) = rx.recv().await {
                let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
                    .await
                    .unwrap();
                tracing::info!("Listening to 127.0.0.1:{}", port);

                if let Ok((mut incoming, addr)) = listener.accept().await {
                    tracing::info!("Accept incoming from {addr:?}");
                    if conn
                        .write_all(&bincode::serialize(&packet::Packet::DataForward).unwrap())
                        .await
                        .is_ok()
                    {
                        tracing::trace!("copy bidirectional data: incoming, conn");
                        let _ = tokio::io::copy_bidirectional(&mut incoming, &mut conn).await;
                    }
                }
            }
        });

        ControlChannel {
            domain_port,
            data_tx: tx,
        }
    }
}

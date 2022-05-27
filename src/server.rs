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
                if let Err(err) = handle_connection(conn, domains, state).await {
                    tracing::error!("handle_connection: {err:?}");
                }
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
    data_tx: mpsc::Sender<TcpStream>,
}

impl ControlChannel {
    pub fn new(
        mut conn: TcpStream,
        domain_port: DomainPort,
        domains: Arc<Mutex<Vec<DomainPort>>>,
    ) -> Self {
        let (tx, mut rx): (_, mpsc::Receiver<TcpStream>) = mpsc::channel(32);
        let (visitor_tx, mut visitor_rx) = mpsc::channel(32);
        let (close_tx, mut close_rx) = tokio::sync::oneshot::channel();

        // Push domain back to domain pools when a client connection is closed.
        // The client is expected to send a Heartbeat every 500 ms.
        let domains = Arc::clone(&domains);
        let dp = domain_port.clone();
        tokio::spawn(async move {
            loop {
                let mut buf = vec![0u8; 1];
                let res = conn.read_exact(&mut buf).await;
                if let Err(err) = res {
                    tracing::error!("receive error: {err}");
                    let mut domains_guard = domains.lock().await;
                    let domain_port = dp.clone();
                    domains_guard.push(domain_port);

                    let port = dp.1;
                    let _ = close_tx.send(port);
                    break;
                }
            }
        });

        // Spawn a TCP socket to listen to the public internet traffic
        // at the assigned port of the domain.
        //
        // Without this, our server can't be receiving any requests from
        // the specified domain.
        let port = domain_port.1;
        tokio::spawn(async move {
            let listener = TcpListener::bind(format!("127.0.0.1:{port}"))
                .await
                .unwrap();
            tracing::info!("Listening to 127.0.0.1:{}", port);

            loop {
                tokio::select! {
                    Ok(port_recv) = &mut close_rx => {
                        tracing::warn!("{port}: receive closed for port: {port_recv}");
                        break;
                    },
                    Ok((incoming, _addr)) = listener.accept() => {
                        let _ = visitor_tx.send(incoming).await;
                    }

                }
            }
        });

        // Listen for incoming requests that is forward to the visitor_rx channel
        // by the TCP server above.
        //
        // If a request is received, listen to the data connection. There should
        // always have at least one connection at the time being as our client code
        // always create a Data Channel once the previous request is completed.
        //
        // If both are found, begin proxying request and response on both side.
        tokio::spawn(async move {
            loop {
                if let Some(mut incoming) = visitor_rx.recv().await {
                    tracing::info!("Accept incoming from visitor_rx");
                    if let Some(mut conn) = rx.recv().await {
                        let packet = bincode::serialize(&packet::Packet::DataForward).unwrap();
                        if conn.write_all(&packet).await.is_ok() {
                            tracing::trace!("copy bidirectional data: incoming, conn");
                            let result =
                                tokio::io::copy_bidirectional(&mut incoming, &mut conn).await;

                            tracing::trace!("result: {result:?}");
                        }
                    }
                }
            }
        });

        ControlChannel { data_tx: tx }
    }
}

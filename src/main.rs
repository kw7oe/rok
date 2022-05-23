use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};

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
    println!("Listening on TCP: 127.0.0.1:3001");
    loop {
        if let Ok((conn, _)) = listener.accept().await {
            println!("Accpeting new client...");
            let state = state.clone();
            let domains = domain_to_port.clone();
            tokio::spawn(async move {
                let _ = handle_connection(conn, domains, state).await;
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
            let mut domains = domains.lock().await;

            let domain_port = domains.pop();

            if domain_port.is_none() {
                println!("oops no more domain available, ignore you");
                return Ok(());
            }

            let domain_port = domain_port.unwrap();
            let domain = domain_port.0.clone();
            let success = packet::Packet::Success(domain.clone());
            conn.write_all(&bincode::serialize(&success).unwrap())
                .await?;
            println!("sent success msg");

            buffer.advance(bytes_len);
            conn.read_buf(&mut buffer).await?;
            if let packet::Packet::Ack = packet::Packet::parse(&buffer) {
                println!("receive ack from client");

                let mut state = state.write().await;

                // Ask client to create a data channel.
                // let create_data = &bincode::serialize(&packet::Packet::CreateData).unwrap();
                // conn.write_all(create_data).await?;
                // println!("sent create data msg");
                let cc = ControlChannel::new(conn, domain_port);
                state.insert(domain, cc);
            }
        }
        packet::Packet::DataInit(domain) => {
            let state = state.write().await;

            if let Some(cc) = state.get(&domain) {
                let listener = TcpListener::bind(format!("127.0.0.1:{}", cc.domain_port.1))
                    .await
                    .unwrap();
                println!("Listening to 127.0.0.1:{}", cc.domain_port.1);

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
        }
        _ => {
            println!("unexpected packet: {packet:?}");
        }
    }
    buffer.advance(bytes_len);

    Ok(())
}

struct ControlChannel {
    conn: TcpStream,
    domain_port: DomainPort,
}

impl ControlChannel {
    pub fn new(conn: TcpStream, domain_port: DomainPort) -> Self {
        ControlChannel { conn, domain_port }
    }
}

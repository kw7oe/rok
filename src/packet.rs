use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Packet {
    Ack,
    Init,
    Success(String),
    CreateData,
    DataInit(String),
    DataForward,
}

impl Packet {
    pub fn parse(bytes: &[u8]) -> Self {
        bincode::deserialize(bytes).unwrap()
    }
}

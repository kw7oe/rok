#[derive(Debug)]
pub enum Packet {
    Init(String),
    Success,
}

impl Packet {
    pub fn parse(bytes: &[u8]) -> Self {
        println!("{:?}", bytes);
        if bytes == b"0" {
            Packet::Init("test.rok.me".to_string())
        } else {
            Packet::Success
        }
    }
}

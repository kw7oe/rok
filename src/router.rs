use std::collections::HashMap;

pub(crate) struct ProxyClient {
    client: reqwest::Client,
    addr: String,
}

impl ProxyClient {
    pub fn new(addr: &str) -> Self {
        ProxyClient {
            client: reqwest::Client::new(),
            addr: addr.to_string(),
        }
    }

    pub async fn add_routes(&self, domain: &str, target: &str) {
        let mut map = HashMap::new();
        map.insert(domain, target);

        self.client
            .post(format!("{}/admin/routes", self.addr))
            .json(&map)
            .send()
            .await
            .expect("should add routes to proxy server successfully");
    }
}

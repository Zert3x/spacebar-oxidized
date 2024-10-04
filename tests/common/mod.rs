use std::ops::{Deref, DerefMut};
use std::thread::sleep;
use std::time::Duration;

use rand::Rng;
use sqlx::migrate::MigrateDatabase;
use sqlx::Postgres;
use symfonia::api;

pub struct Server {
    inner: symfonia::Server,
}

impl Deref for Server {
    type Target = symfonia::Server;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Server {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Server {
    pub async fn new(suffix: u32, gateway_port: u16, api_port: u16) -> Self {
        let args = symfonia::Args { migrate: false };

        std::env::set_var("DATABASE_NAME", format!("symfonia-test-{suffix}"));
        std::env::set_var("MODE", "VERBOSE");
        std::env::set_var("GATEWAY_BIND", format!("127.0.0.1:{gateway_port}"));
        std::env::set_var("API_BIND", format!("127.0.0.1:{api_port}"));

        Postgres::create_database(&format!(
            "postgres://symfonia:symfonia@localhost:5432/symfonia-test-{suffix}"
        ))
        .await
        .unwrap();

        let inner = symfonia::Server::new(args).await.unwrap();
        Self { inner }
    }
}

#[tokio::test]
async fn test_server_struct() {
    let mut rng = rand::thread_rng();
    let suffix = rng.gen_range(1000..=100000);
    let gateway_port = rng.gen_range(32768..=65535);
    let mut api_port = rng.gen_range(32768..=65535);
    println!("Gateway port: {}", gateway_port);
    println!("API port: {}", api_port);
    println!("Suffix: {}", suffix);
    while gateway_port == api_port {
        api_port = rng.gen_range(32768..=65535);
    }
    let server = Server::new(suffix, gateway_port, api_port).await;
    server.start().await.unwrap()
}

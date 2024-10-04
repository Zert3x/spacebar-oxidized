use std::ops::{Deref, DerefMut};
use std::thread::sleep;
use std::time::Duration;

use futures::executor::block_on;
use futures::FutureExt;
use rand::Rng;
use sqlx::migrate::MigrateDatabase;
use sqlx::Postgres;
use symfonia::api;
use tokio::task::JoinHandle;

pub struct Server {
    inner: symfonia::Server,
    task: Option<JoinHandle<()>>,
    suffix: u32,
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
        Self {
            inner,
            task: None,
            suffix,
        }
    }

    pub async fn start(mut self) -> Self {
        dbg!("Starting server");
        let server_clone = self.inner.clone();
        let task = tokio::spawn(async move { server_clone.clone().start().await.unwrap() });
        self.task = Some(task);
        self
    }

    pub async fn stop(mut self) {
        dbg!("Stopping server");
        self.task.unwrap().abort();
        self.task = None;
        Postgres::force_drop_database(&format!(
            "postgres://symfonia:symfonia@localhost:5432/symfonia-test-{}",
            self.suffix
        ))
        .await
        .unwrap()
    }
}

#[tokio::test(flavor = "multi_thread")]
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
    let server = Server::new(suffix, gateway_port, api_port)
        .await
        .start()
        .await;
    for _ in 0..10 {
        sleep(Duration::from_secs(1));
    }
    server.stop().await;
    panic!()
}

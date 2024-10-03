use std::ops::{Deref, DerefMut};
use std::thread::sleep;
use std::time::Duration;

use testcontainers::{
    core::{ImageExt, IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage,
};

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
    pub async fn new() -> Self {
        let args = symfonia::Args { migrate: false };

        let postgres = GenericImage::new("postgres", "17-alpine")
            .with_exposed_port(5432.tcp())
            .with_wait_for(WaitFor::message_on_stdout(
                "database system is ready to accept connections",
            ))
            .with_env_var("POSTGRES_USER", "symfonia")
            .with_env_var("POSTGRES_PASSWORD", "symfonia")
            .with_env_var("POSTGRES_DB", "symfonia")
            .start()
            .await
            .unwrap();

        let host = postgres.get_host().await.unwrap();
        let host_port = postgres.get_host_port_ipv4(5432).await.unwrap();
        std::env::set_var("DATABASE_HOST", "localhost");
        std::env::set_var("DATABASE_PORT", &host_port.to_string());
        std::env::set_var("DATABASE_USERNAME", "symfonia");
        std::env::set_var("DATABASE_PASSWORD", "symfonia");
        std::env::set_var("DATABASE_NAME", "symfonia");

        let inner = symfonia::Server::new(args).await.unwrap();
        Self { inner }
    }
}

#[tokio::test]
async fn test_server_struct() {
    Server::new().await;
    sleep(Duration::from_secs(10));
}

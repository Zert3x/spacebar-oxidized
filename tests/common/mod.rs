use std::ops::{Deref, DerefMut};

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
        let inner = symfonia::Server::new(args).await.unwrap();
        Self { inner }
    }
}

#[tokio::test]
async fn test_server_struct() {
    Server::new().await;
}

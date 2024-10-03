use std::{thread::sleep, time::Duration};

#[tokio::main]
pub(crate) async fn main() -> Result<(), symfonia::errors::Error> {
    symfonia::Server::new().await?;
    loop {
        sleep(Duration::from_secs(5));
    }
}

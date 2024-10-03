use std::{thread::sleep, time::Duration};

use clap::Parser;
use symfonia::Args;

#[tokio::main]
pub(crate) async fn main() -> Result<(), symfonia::errors::Error> {
    let args = Args::parse();
    symfonia::Server::new(args).await?;
    loop {
        sleep(Duration::from_secs(5));
    }
}

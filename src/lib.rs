/*
 *  This Source Code Form is subject to the terms of the Mozilla Public
 *  License, v. 2.0. If a copy of the MPL was not distributed with this
 *  file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#![allow(unused)] // TODO: Remove, I just want to clean up my build output
#![allow(special_module_name)]

/**
This library is not meant to be used in your project. It is a part of the `symfonia` binary
and exists to allow for integration testing of the binary. The library is not covered by the
semver guarantees of the binary and may change at any time without warning.
*/
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chorus::types::Snowflake;
use clap::Parser;

use gateway::{ConnectedUsers, Event};
use log::LevelFilter;
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        rolling_file::{
            policy::compound::{
                roll::delete::DeleteRoller, trigger::size::SizeTrigger, CompoundPolicy,
            },
            RollingFileAppender,
        },
    },
    config::{Appender, Logger, Root},
    encode::pattern::PatternEncoder,
    filter::Filter,
    Config,
};
use parking_lot::RwLock;
use pubserve::Publisher;
use tokio::sync::Mutex;

pub mod api;
pub mod cdn;
pub mod database;
pub mod errors;
pub mod gateway;
pub(crate) mod main;
pub mod util;

pub type SharedEventPublisher = Arc<RwLock<Publisher<Event>>>;
pub type EventPublisherMap = HashMap<Snowflake, SharedEventPublisher>;
pub type SharedEventPublisherMap = Arc<RwLock<EventPublisherMap>>;
pub type WebSocketReceive =
    futures::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>;
pub type WebSocketSend = futures::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    tokio_tungstenite::tungstenite::Message,
>;

pub fn eq_shared_event_publisher(a: &SharedEventPublisher, b: &SharedEventPublisher) -> bool {
    let a = a.read();
    let b = b.read();
    *a == *b
}

#[derive(Debug)]
struct LogFilter;

impl Filter for LogFilter {
    fn filter(&self, record: &log::Record) -> log4rs::filter::Response {
        if record.target().starts_with("symfonia") {
            log4rs::filter::Response::Accept
        } else {
            log4rs::filter::Response::Reject
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "false")]
    migrate: bool,
}

/// Start the server.
pub async fn start_server() -> Result<(), crate::errors::Error> {
    main::main()
}

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

mod api;
mod cdn;
mod database;
mod errors;
mod gateway;
mod util;

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

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let stdout = ConsoleAppender::builder()
        .target(Target::Stdout)
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} | {h({l:<6.6})} | {t:<35} | {m}{n}",
        )))
        .build();

    let api_log = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build(
            "log/api.log",
            Box::new(CompoundPolicy::new(
                Box::new(SizeTrigger::new(1024 * 1024 * 30)),
                Box::new(DeleteRoller::new()),
            )),
        )
        .unwrap();

    let cdn_log = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build(
            "log/cdn.log",
            Box::new(CompoundPolicy::new(
                Box::new(SizeTrigger::new(1024 * 1024 * 30)),
                Box::new(DeleteRoller::new()),
            )),
        )
        .unwrap();

    let gateway_log = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build(
            "log/gateway.log",
            Box::new(CompoundPolicy::new(
                Box::new(SizeTrigger::new(1024 * 1024 * 30)),
                Box::new(DeleteRoller::new()),
            )),
        )
        .unwrap();

    let config = Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(LogFilter))
                .build("stdout", Box::new(stdout)),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(LogFilter))
                .build("api", Box::new(api_log)),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(LogFilter))
                .build("cdn", Box::new(cdn_log)),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(LogFilter))
                .build("gateway", Box::new(gateway_log)),
        )
        //.logger(Logger::builder().build("symfonia::db", LevelFilter::Info))
        //.logger(Logger::builder().build("symfonia::cfg", LevelFilter::Info))
        .logger(
            Logger::builder()
                .appender("api")
                .build("symfonia::api", LevelFilter::Info),
        )
        .logger(
            Logger::builder()
                .appender("cdn")
                .build("symfonia::cdn", LevelFilter::Info),
        )
        .logger(
            Logger::builder()
                .appender("gateway")
                .build("symfonia::gateway", LevelFilter::Info),
        )
        .build(Root::builder().appender("stdout").build({
            let mode = std::env::var("MODE").unwrap_or("DEBUG".to_string());
            match mode.as_str() {
                "DEBUG" => LevelFilter::Debug,
                "PRODUCTION" => LevelFilter::Warn,
                "VERBOSE" => LevelFilter::Trace,
                _ => LevelFilter::Debug,
            }
        }))
        .unwrap();

    let _handle = log4rs::init_config(config).unwrap();

    log::info!(target: "symfonia", "Starting up Symfonia");
}

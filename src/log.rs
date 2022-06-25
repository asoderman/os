use crate::dev::serial::set_serial_logger;

use log::set_max_level;
use log::LevelFilter;

const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::Info;

fn from_str(s: &str) -> LevelFilter {
    match s.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Off
    }
}

pub fn init() {
    set_serial_logger();
    set_max_level(option_env!("LOG_LEVEL").map(|s| from_str(s)).unwrap_or(DEFAULT_LOG_LEVEL));
}

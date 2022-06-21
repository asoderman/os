use crate::dev::serial::set_serial_logger;

use log::set_max_level;

pub fn init() {
    set_serial_logger();
    set_max_level(log::LevelFilter::Trace);
}

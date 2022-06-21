use spin::Mutex;
use core::fmt::{Write, Error};

use x86_64::instructions::port::PortWriteOnly;
use log::Log;

static SERIAL_OUT: Mutex<Serial> = Mutex::new(Serial::com1());
static SERIAL_REF: SerialRef = SerialRef(&SERIAL_OUT);

const COM1: u16 = 0x3F8;

pub struct Serial {
    port: u16
}

impl Serial {
    pub const fn com1() -> Self {
        Self {
            port: COM1,
        }
    }

    fn write(&self, s: &str) {
        let mut p = PortWriteOnly::new(self.port);
        for b in s.bytes() {
            unsafe {
                p.write(b);
            }
        }
    }
}

impl Write for Serial {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        self.write(s);
        Ok(())
    }
}

struct SerialRef(&'static Mutex<Serial>);

pub fn set_serial_logger() {
    log::set_logger(&SERIAL_REF).unwrap();
}

impl Log for SerialRef {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        crate::heap::HEAP_READY.is_completed()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            println!("[{}]:{}", record.level(), record.args());
        }
    }

    fn flush(&self) {
        ()
    }
}

pub fn print(args: core::fmt::Arguments) {
    crate::interrupt::without_interrupts(|| {
        SERIAL_OUT.lock().write_fmt(args).unwrap()
    });
}

pub fn write_serial_out(s: &str) {
    let mut p = PortWriteOnly::new(COM1);
    for b in s.bytes() {
        unsafe {
            p.write(b);
        }
    }
}

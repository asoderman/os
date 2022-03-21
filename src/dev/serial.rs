use lazy_static::lazy_static;
use spin::Mutex;
use core::{fmt::{Write, Error}, convert::Infallible};

use x86_64::instructions::port::PortWriteOnly;

pub static SERIAL_OUT: Mutex<Serial> = Mutex::new(Serial::com1());

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

pub fn write_serial_out(s: &str) {
    let mut p = PortWriteOnly::new(COM1);
    for b in s.bytes() {
        unsafe {
            p.write(b);
        }
    }
}

use spin::Mutex;
use core::fmt::{Write, Error, Display};

use x86_64::instructions::port::PortWriteOnly;
use log::{Log, Level};

use crate::fs::GenericFile;

static SERIAL_OUT: Mutex<Serial> = Mutex::new(Serial::com1());
static SERIAL_REF: SerialRef = SerialRef(&SERIAL_OUT);

const COM1: u16 = 0x3F8;

#[derive(Debug)]
enum Color<'s, T: Display> {
    Red(&'s T),
    Green(&'s T),
    Blue(&'s T),
    Cyan(&'s T),
    Yellow(&'s T),
    //White(&'s T),
}

impl<'s, T: Display> Color<'s, T> {
    fn escape_code(&self) -> &'static str {
        match self {
            Self::Red(_) => "\x1b[31m",
            Self::Green(_) => "\x1b[32m",
            Self::Yellow(_) => "\x1b[33m",
            Self::Blue(_) => "\x1b[34m",
            Self::Cyan(_) => "\x1b[36m",
            //Self::White(_) => "\x1b[37m",
        }
    }

    fn inner(&self) -> &'s T {
        match self {
            Self::Red(inner) => inner,
            Self::Green(inner) => inner,
            Self::Yellow(inner) => inner,
            Self::Blue(inner) => inner,
            Self::Cyan(inner) => inner,
            //Self::White(inner) => inner,
        }
    }

    fn off() -> &'static str {
        "\x1b[0m"
    }
}

impl<'s, T: Display> Display for Color<'s, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{escape_code}{inner}{off}", 
            escape_code = self.escape_code(),
            inner = self.inner(),
            off = Self::off()
        )
    }
}

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
            let level = record.level();
            let c_level = match record.level() {
                Level::Error => { Color::Red(&level) },
                Level::Warn => { Color::Yellow(&level) },
                Level::Info => { Color::Blue(&level) },
                Level::Debug => { Color::Green(&level) },
                Level::Trace => { Color::Cyan(&level) }
            };

            println!("[{}:{}]: {}", crate::proc::try_pid().unwrap_or(0), c_level, record.args());
        }
    }

    fn flush(&self) {
        ()
    }
}

/// Constructs a generic device where the read and write invoke the serial port
pub fn generic_serial_device() -> GenericFile {
    let mut file = GenericFile::default();

    file.read_impl = |_buffer| { todo!() };
    file.write_impl = |buffer| {
        SERIAL_OUT.lock().write(core::str::from_utf8(buffer).unwrap_or("SERIAL ERROR"));
        Ok(buffer.len())
    };

    file
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

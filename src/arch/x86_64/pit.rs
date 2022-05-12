use lazy_static::lazy_static;
use spin::{Mutex, MutexGuard};
use x86_64::instructions::port::Port;

const CHANNEL_0: u16 = 0x40;
const CHANNEL_1: u16 = 0x41;
const CHANNEL_2: u16 = 0x42;
const MODE_COMMAND_REG: u16 = 0x43;

// 1000.15255658 hz
const RELOAD_VALUE_1KHZ: u16 = 1193;

const fn PIT_TICK_RATE_HZ(reload_value: usize) -> usize {
    1193182 / reload_value
}

lazy_static! {
    pub static ref PIT: Mutex<Pit> = Mutex::new(
    Pit(Port::new(CHANNEL_0),
        Port::new(CHANNEL_1),
        Port::new(CHANNEL_2),
        Port::new(MODE_COMMAND_REG)).init()
    );
}

pub struct Pit(Port<u8>,Port<u8>,Port<u8>,Port<u8>,);

impl Pit {

    pub fn init(mut self) -> Self {
        unsafe {
            self.set_ms_tick_rate();
        }

        self

    }

    unsafe fn set_reload_value(&mut self, value: u16) {
        // set low byte
        self.0.write((value&0xFF) as u8);
        // set high byte
        self.0.write(((value&0xFF00) >> 8) as u8);
    }

    unsafe fn set_ms_tick_rate(&mut self) {
        self.set_reload_value(RELOAD_VALUE_1KHZ);
    }

    /// Wait for the specified ms to pass via polling
    pub fn wait(&mut self, ms: usize) {
        let start = self.read_current();
        let mut ms_elapsed = 0;
        let mut ticks_elapsed = 0;
        let mut last = u64::MAX;

        loop {
            let new = self.read_current();
            if last == new as u64 { continue };

            ticks_elapsed += 1;

            if new == start && ticks_elapsed != 0 {
                ms_elapsed += 1;

                if ms_elapsed == ms {
                    return;
                }
            }

            last = new as u64;
        }
    }

    /// Read the current value of the Pit counter
    fn read_current(&mut self) -> u16 {
        unsafe {
            self.3.write(0b0000000);

            let mut v = self.0.read() as u16;
            v |= (self.0.read() as u16) << 8;
            v
        }
    }
}

pub fn pit<'p>() -> MutexGuard<'p, Pit> {
    PIT.lock()
}

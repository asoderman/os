use crate::time::{Time, Date, DateTime};

use x86_64::instructions::port::{PortRead, PortWrite};

const ADDR_PORT: u16 = 0x70;
const DATA_PORT: u16 = 0x71;

const STATUS_A: u8 = 0x0A;
const STATUS_B: u8 = 0x0B;

const SECONDS: u8 = 0x00;
const MINUTES: u8 = 0x02;
const HOURS: u8 = 0x04;
const DAY_OF_MONTH: u8 = 0x07;
const MONTH: u8 = 0x08;
const YEAR: u8 = 0x09;
const CENTURY: u8 = 0x32;

/// A basic driver to read the RTC
pub struct Rtc {}

impl Rtc {
    pub fn now() -> DateTime {
        loop {
            let time;
            'update_loop: loop {
                if !update_in_progress(Self::status_a()) {
                    let t = Self::read_date_time();
                    if t == Self::read_date_time() && !update_in_progress(Self::status_a()) {
                        time = t;
                        break 'update_loop;
                    }
                }
            }

            // One final check to see if we got a good read
            if time.time.seconds < 60 && time.time.minutes < 60 && time.time.hours < 24 { 
                return time;
            }
        }
    }

    pub fn current_time() -> Time {
        Self::now().time
    }

    /// Perform a raw RTC read
    fn read_date_time() -> DateTime {
        let seconds;
        let minutes;
        let hours;
        let day_of_month;
        let month;
        let year;

        if is_bcd(Self::status_b()) {
            seconds = from_bcd(Self::seconds());
            minutes = from_bcd(Self::minutes());
            hours = from_bcd(Self::hours());

            day_of_month = from_bcd(Self::day_of_month());
            month = from_bcd(Self::month());

            let century = Self::century();
            year = if century == 0 {
                from_bcd(Self::year()) as u16
            } else {
                from_bcd(Self::year()) as u16 + (from_bcd(century) as u16 * 100)
            };
        } else {
            seconds = Self::seconds();
            minutes = Self::minutes();
            hours = Self::hours();

            day_of_month = Self::day_of_month();
            month = Self::day_of_month();
            let century = Self::century();
            year = if century == 0 {
                Self::year() as u16
            } else {
                Self::year() as u16 + century as u16
            };

        }

        if is_24hr(Self::status_b()) {
            todo!("Handle 24 hour time!")
        }

        let time = Time {
            seconds,
            minutes,
            hours,
        };

        let date = Date {
            day: day_of_month,
            month,
            year
        };

        DateTime {
            date,
            time
        }
    }

    fn read(addr: u8) -> u8 {
        // FIXME: This can race if two threads want to read at the same time
        unsafe {
            PortWrite::write_to_port(ADDR_PORT, addr);
            PortRead::read_from_port(DATA_PORT)
        }
    }

    fn status_a() -> u8 {
        Self::read(STATUS_A)
    }

    fn status_b() -> u8 {
        Self::read(STATUS_B)
    }

    fn seconds() -> u8 {
        Self::read(SECONDS)
    }

    fn minutes() -> u8 {
        Self::read(MINUTES)
    }

    fn hours() -> u8 {
        Self::read(HOURS)
    }

    fn day_of_month() -> u8 {
        Self::read(DAY_OF_MONTH)
    }

    fn month() -> u8 {
        Self::read(MONTH)
    }

    fn year() -> u8 {
        Self::read(YEAR)
    }

    fn century() -> u8 {
        Self::read(CENTURY)
    }
}

fn from_bcd(value: u8) -> u8 {
    (value & 0xF) + ((value / 16) * 10)
}

/// Check if the update in progress flag is set
fn update_in_progress(status: u8) -> bool {
    const MASK: u8 = 1 << 7;
    status & MASK != 0
}

/// Check if the RTC is producing binary coded decimal
fn is_bcd(status: u8) -> bool {
    const MASK: u8 = 1 << 2;
    status & MASK != MASK
}

fn is_24hr(status: u8) -> bool {
    const MASK: u8 = 1 << 1;
    status & MASK != 0
}

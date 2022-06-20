use core::ops::{Add, Sub};
use core::cmp::{Ord, PartialOrd};
use core::fmt::Display;

use crate::arch::x86_64::timers::rtc::Rtc;

const SECOND: usize = 1;
const MINUTE: usize = 60 * SECOND;
const HOUR: usize = 60 * MINUTE;
#[allow(dead_code)]
const DAY: usize = 24 * HOUR;

const _MONTH: usize = 30 * DAY; // 30.44
const _YEAR: usize = 365 * DAY; // 365.24

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Seconds(pub usize);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnixEpoch(pub u64);

impl From<DateTime> for UnixEpoch {
    fn from(dt: DateTime) -> Self {
        let mut epoch = (dt.date.year as u64 - 1970) * 31_536_000;

        let mut leap_days = (dt.date.year as u64 - 1972) / 4 + 1;

        if dt.date.year % 4 == 0 && dt.date.month <= 2 {
            leap_days -= 1;
        }

        epoch += leap_days * 86_400;

        match dt.date.month {
            2 => epoch += 2_678_400,
            3 => epoch += 5_097_600,
            4 => epoch += 7_776_000,
            5 => epoch += 10_368_000,
            6 => epoch += 13_046_400,
            7 => epoch += 15_638_400,
            8 => epoch += 18_316_800,
            9 => epoch += 20_995_200,
            10 => epoch += 23_587_200,
            11 => epoch += 26_265_600,
            12 => epoch += 28_857_600,
            _ => (),
        }

        epoch += (dt.date.day as u64 - 1) * 86_400;
        epoch += dt.time.hours as u64 * 3600;
        epoch += dt.time.minutes as u64 * 60;
        epoch += dt.time.seconds as u64;

        UnixEpoch(epoch)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Date {
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

impl Display for Date {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let m = match self.month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => ""
        };

        write!(f, "{month} {day}, {year}", month=m, day=self.day, year=self.year)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DateTime {
    pub date: Date,
    pub time: Time
}

impl Display for DateTime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} {}", self.date, self.time)
    }
}

impl DateTime {
    pub fn now() -> Self {
        Rtc::now()
    }

    #[allow(dead_code)]
    pub fn utc_now() -> UnixEpoch {
        Self::now().into()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Time {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
}

impl Display for Time {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}:{}:{}", self.hours, self.minutes, self.seconds)
    }
}

impl Into<Seconds> for Time {
    fn into(self) -> Seconds {
        Seconds(
            self.seconds as usize
            + (self.minutes as usize * MINUTE)
            + (self.hours as usize * HOUR)
            )
    }
}

impl From<Seconds> for Time {
    fn from(seconds: Seconds) -> Self {
        Self {
            seconds: (seconds.0 % 60) as u8,
            minutes: ((seconds.0 / MINUTE) % 60) as u8,
            hours: ((seconds.0 / HOUR) % 24) as u8,
        }
    }
}

impl Ord for Time {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let self_seconds: Seconds = (*self).into();
        let other_seconds: Seconds = (*other).into();

        self_seconds.cmp(&other_seconds)
    }
}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: Into<Seconds>> Add<S> for Time {
    type Output = Self;

    fn add(self, rhs: S) -> Self::Output {
        let lhs_as_seconds: Seconds = self.into();
        let rhs_as_seconds: Seconds = rhs.into();

        Seconds(lhs_as_seconds.0 + rhs_as_seconds.0).into()
    }
}

impl<S: Into<Seconds>> Sub<S> for Time {
    type Output = Time;

    fn sub(self, rhs: S) -> Self::Output {
        let lhs_as_seconds: Seconds = self.into();
        let rhs_as_seconds: Seconds = rhs.into();

        Seconds(lhs_as_seconds.0 - rhs_as_seconds.0).into()
    }
}

impl Time {
    pub fn now() -> Time {
        Rtc::current_time()
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test_case]
    fn test_min_to_seconds() {
        let mut min = Time::default();

        min.seconds = 14;

        min.minutes = 19;

        let min_as_seconds: Seconds = min.into();

        assert_eq!(min_as_seconds, Seconds((19 * 60) + 14));
        let min_as_time: Time = min_as_seconds.into();
        assert_eq!(min_as_time, min);
    }

    #[test_case]
    fn test_hour_to_seconds() {
        let mut hour = Time::default();

        hour.seconds = 14;

        hour.hours = 19;

        let as_seconds: Seconds = hour.into();

        assert_eq!(as_seconds, Seconds((19 * 60 * 60) + 14));
        let as_time: Time = as_seconds.into();
        assert_eq!(as_time, hour);
    }

    #[test_case]
    fn test_time_to_seconds() {
        let mut time = Time::default();

        time.seconds = 14;

        time.minutes = 30;

        time.hours = 19;

        let as_seconds: Seconds = time.into();

        assert_eq!(as_seconds, Seconds((19 * 60 * 60) + (30 * 60) + 14));
        let as_time: Time = as_seconds.into();
        assert_eq!(as_time, time);
    }

    #[test_case]
    fn test_time_cmp() {
        let start = Time {
            seconds: 57,
            minutes: 22,
            hours: 19,
        };
        let end = Time {
            seconds: 2,
            minutes: 23,
            hours: 19,
        };

        assert!(end > start);
    }

    #[test_case]
    fn test_time_addition() {
        let mut time = Time::default();

        time.minutes = 59;
        time.seconds = 55;

        let res = time + Seconds(6);

        let expected = Time {
            seconds: 1,
            minutes: 0,
            hours: 1,
            ..Default::default()
        };
        assert_eq!(res, expected);
    }
}

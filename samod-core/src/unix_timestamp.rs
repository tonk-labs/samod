use std::{
    ops::{Add, AddAssign, Sub},
    time::Duration,
};

#[cfg(not(all(target_arch = "wasm32", feature = "wasm-browser")))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(all(target_arch = "wasm32", feature = "wasm-browser"))]
use js_sys;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixTimestamp {
    millis: u128,
}

impl std::fmt::Display for UnixTimestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.millis)
    }
}

impl std::fmt::Debug for UnixTimestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.millis)
    }
}

impl UnixTimestamp {
    pub fn now() -> Self {
        #[cfg(all(target_arch = "wasm32", feature = "wasm-browser"))]
        {
            Self {
                millis: js_sys::Date::now() as u128,
            }
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "wasm-browser")))]
        {
            Self {
                millis: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            }
        }
    }

    pub fn as_millis(&self) -> u128 {
        self.millis
    }
}

impl From<UnixTimestamp> for i64 {
    fn from(ts: UnixTimestamp) -> i64 {
        ts.millis as i64
    }
}

impl AddAssign<Duration> for UnixTimestamp {
    fn add_assign(&mut self, rhs: Duration) {
        self.millis += rhs.as_millis();
    }
}

impl Add<Duration> for UnixTimestamp {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self {
            millis: self.millis + rhs.as_millis(),
        }
    }
}

impl Sub<Duration> for UnixTimestamp {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self {
            millis: self.millis - rhs.as_millis(),
        }
    }
}

impl Sub<UnixTimestamp> for UnixTimestamp {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        let diff = self.millis - rhs.millis;
        Duration::from_millis(diff as u64)
    }
}

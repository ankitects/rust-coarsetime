use super::duration::*;
#[allow(unused_imports)]
use super::helpers::*;
#[allow(unused_imports)]
use std::mem::MaybeUninit;
use std::ops::*;
#[allow(unused_imports)]
use std::ptr::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// A measurement of a monotonically increasing clock. Opaque and useful only with `Duration`.
#[derive(Copy, Clone, Debug, Hash, Ord, Eq, PartialOrd, PartialEq)]
pub struct Instant(u64);

static RECENT: AtomicU64 = AtomicU64::new(0);

#[cfg(windows)]
extern "system" {
    pub fn GetTickCount() -> libc::c_ulong;
}

#[cfg(any(all(feature = "sierra", target_os = "macos"), target_os = "freebsd"))]
#[allow(non_camel_case_types)]
type clockid_t = libc::c_int;

#[cfg(all(feature = "sierra", target_os = "macos"))]
const CLOCK_MONOTONIC_RAW_APPROX: clockid_t = 5;

#[cfg(all(feature = "sierra", target_os = "macos"))]
extern "system" {
    pub fn clock_gettime_nsec_np(clk_id: clockid_t) -> u64;
}

#[cfg(target_os = "freebsd")]
const CLOCK_MONOTONIC_FAST: clockid_t = 12;

impl Instant {
    /// Returns an instant corresponding to "now"
    ///
    /// This function also updates the stored instant.
    pub fn now() -> Instant {
        let now = Self::_now();
        Self::_update(now);
        Instant(now)
    }

    /// Returns an instant corresponding to the latest update
    pub fn recent() -> Instant {
        match Self::_recent() {
            0 => Instant::now(),
            recent => Instant(recent),
        }
    }

    /// Update the stored instant
    ///
    /// This function should be called frequently, for example in an event loop or using an
    /// `Updater` task.
    pub fn update() {
        let now = Self::_now();
        Self::_update(now);
    }

    /// Returns the amount of time elapsed from another instant to this one
    #[inline]
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        *self - earlier
    }

    /// Returns the amount of time elapsed between the this instant was created and the latest
    /// update
    #[inline]
    pub fn elapsed_since_recent(&self) -> Duration {
        Self::recent() - *self
    }

    /// Returns the amount of time elapsed since this instant was created
    ///
    /// This function also updates the stored instant.
    #[inline]
    pub fn elapsed(&self) -> Duration {
        Self::now() - *self
    }

    #[doc(hidden)]
    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn _now() -> u64 {
        let mut tp = MaybeUninit::<libc::timespec>::uninit();
        let tp = unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC_COARSE, tp.as_mut_ptr());
            tp.assume_init()
        };
        _timespec_to_u64(tp.tv_sec as u64, tp.tv_nsec as u32)
    }

    #[cfg(all(feature = "sierra", target_os = "macos"))]
    fn _now() -> u64 {
        let nsec = unsafe { clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW_APPROX) };
        _nsecs_to_u64(nsec)
    }

    #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
    fn _now() -> u64 {
        let mut tp = MaybeUninit::<libc::timespec>::uninit();
        let tp = unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC_FAST, tp.as_mut_ptr());
            tp.assume_init()
        };
        _timespec_to_u64(tp.tv_sec as u64, tp.tv_nsec as u32)
    }

    #[cfg(all(
        unix,
        not(any(
            all(feature = "sierra", target_os = "macos"),
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly"
        ))
    ))]
    fn _now() -> u64 {
        let mut tv = MaybeUninit::<libc::timeval>::uninit();
        let tv = unsafe {
            libc::gettimeofday(tv.as_mut_ptr(), null_mut());
            tv.assume_init()
        };
        _timeval_to_u64(tv.tv_sec as u64, tv.tv_usec as u32)
    }

    #[cfg(windows)]
    fn _now() -> u64 {
        let tc = unsafe { GetTickCount() } as u64;
        _millis_to_u64(tc)
    }

    #[cfg(target_os = "wasi")]
    fn _now() -> u64 {
        use wasi::{clock_time_get, CLOCKID_MONOTONIC};
        let nsec =
            unsafe { clock_time_get(CLOCKID_MONOTONIC, 1_000_000).expect("Clock not available") };
        _nsecs_to_u64(nsec)
    }

    #[cfg(not(any(windows, unix, target_os = "wasi")))]
    fn _now() -> u64 {
        panic!("Unsupported target");
    }

    #[inline]
    fn _update(now: u64) {
        RECENT.store(now, Ordering::Relaxed)
    }

    #[inline]
    fn _recent() -> u64 {
        let recent = RECENT.load(Ordering::Relaxed);
        if recent != 0 {
            recent
        } else {
            let now = Self::_now();
            Self::_update(now);
            Self::_recent()
        }
    }
}

impl Default for Instant {
    fn default() -> Instant {
        Self::now()
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    #[cfg(all(
        unix,
        not(any(
            all(feature = "sierra", target_os = "macos"),
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly"
        ))
    ))]
    #[inline]
    fn sub(self, other: Instant) -> Duration {
        Duration::from_u64(self.0.saturating_sub(other.0))
    }

    #[cfg(not(all(
        unix,
        not(any(
            all(feature = "sierra", target_os = "macos"),
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly"
        ))
    )))]
    #[inline]
    fn sub(self, other: Instant) -> Duration {
        Duration::from_u64(self.0 - other.0)
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    #[inline]
    fn sub(self, rhs: Duration) -> Instant {
        Instant(self.0 - rhs.as_u64())
    }
}

impl SubAssign<Duration> for Instant {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    #[inline]
    fn add(self, rhs: Duration) -> Instant {
        Instant(self.0 + rhs.as_u64())
    }
}

impl AddAssign<Duration> for Instant {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

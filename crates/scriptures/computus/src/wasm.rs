//! WebAssembly exports for the reader's browser: the lectionary page computes Pascha
//! and the movable cycle for any year the visitor types, with this same Rust code.
//!
//! > *"Verily, verily, I say unto you, The hour is coming, and now is, when the dead shall
//! > hear the voice of the Son of God: and they that hear shall live."* - John 5:25, KJV
//!
//! The exports take and return `i32` so JavaScript sees plain numbers. Years are
//! clamped to 1..=200,000, where every intermediate value fits in `i64` and every
//! result fits in `i32` (the largest JDN, near year 200,001, is about 74 million).
#![allow(unsafe_code)]

use crate::{Date, jdn_to_gregorian, julian_to_jdn, pascha_jdn, pascha_julian};

fn clamp_year(year: i32) -> i64 {
    i64::from(year.clamp(1, 200_000))
}

fn narrow(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(0)
}

// SAFETY: `no_mangle` only fixes the exported symbol name so the page can call it.
// The function is safe Rust over integers: no pointers, no memory is shared.
/// Julian Day Number of Pascha in `year`.
#[unsafe(no_mangle)]
pub extern "C" fn pascha(year: i32) -> i32 {
    narrow(pascha_jdn(clamp_year(year)))
}

// SAFETY: as above - an exported name over pure integer arithmetic.
/// Julian-calendar month of Pascha in `year`.
#[unsafe(no_mangle)]
pub extern "C" fn pascha_julian_month(year: i32) -> i32 {
    narrow(pascha_julian(clamp_year(year)).month)
}

// SAFETY: as above.
/// Julian-calendar day of Pascha in `year`.
#[unsafe(no_mangle)]
pub extern "C" fn pascha_julian_day(year: i32) -> i32 {
    narrow(pascha_julian(clamp_year(year)).day)
}

fn civil(jdn: i32) -> Date {
    jdn_to_gregorian(i64::from(jdn))
}

// SAFETY: as above.
/// Gregorian year of a JDN.
#[unsafe(no_mangle)]
pub extern "C" fn civil_year(jdn: i32) -> i32 {
    narrow(civil(jdn).year)
}

// SAFETY: as above.
/// Gregorian month of a JDN.
#[unsafe(no_mangle)]
pub extern "C" fn civil_month(jdn: i32) -> i32 {
    narrow(civil(jdn).month)
}

// SAFETY: as above.
/// Gregorian day of a JDN.
#[unsafe(no_mangle)]
pub extern "C" fn civil_day(jdn: i32) -> i32 {
    narrow(civil(jdn).day)
}

// SAFETY: as above.
/// JDN of a Julian-calendar date (for the Julian dates shown beside the civil ones).
#[unsafe(no_mangle)]
pub extern "C" fn julian_jdn(year: i32, month: i32, day: i32) -> i32 {
    narrow(julian_to_jdn(Date {
        year: i64::from(year),
        month: i64::from(month),
        day: i64::from(day),
    }))
}

/// Nothing in this crate can panic (integer arithmetic on clamped inputs); if it ever
/// did, trapping is the only honest answer in a browser.
#[panic_handler]
fn on_panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

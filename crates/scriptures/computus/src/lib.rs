//! # Computus - the Orthodox Paschalion for any year
//!
//! > *"Jesus said unto her, I am the resurrection, and the life: he that believeth in
//! > me, though he were dead, yet shall he live:"* - John 11:25, KJV
//!
//! JESUS CHRIST is the cornerstone and chief builder of this work. Amen.
//!
//! ## What is computed, and from where
//!
//! - **Pascha** by the Julian computus the Orthodox Church keeps, in the form given by
//!   Jean Meeus, *Astronomical Algorithms* (2nd ed., 1998), chapter 8, "Date of Easter",
//!   Julian calendar: `a = Y mod 4`, `b = Y mod 7`, `c = Y mod 19`,
//!   `d = (19c + 15) mod 30`, `e = (2a + 4b - d + 34) mod 7`; the month is
//!   `(d + e + 114) div 31` and the day `((d + e + 114) mod 31) + 1`, Julian.
//! - **Day numbers**: the Julian Day Number of a Julian-calendar date, and the
//!   Gregorian (civil) date of a Julian Day Number, by the integer algorithms of
//!   E. G. Richards (in *Explanatory Supplement to the Astronomical Almanac*, 3rd ed.,
//!   2013, ch. 15). Integer arithmetic only, so year 40,000 is as exact as year 2026.
//! - **The movable cycle** as day offsets from Pascha, each checked against orthocal.info
//!   for 2026 (see `tests` and the site's PROVENANCE).
//!
//! The functions are pure (and `no_std` on WebAssembly), so the same code runs in the static-site build
//! and, compiled to WebAssembly, in the reader's browser.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// A calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    /// Year (astronomical numbering; the site uses years from 1).
    pub year: i64,
    /// Month 1-12.
    pub month: i64,
    /// Day of month 1-31.
    pub day: i64,
}

/// Pascha as a Julian-calendar date (Meeus, ch. 8, Julian algorithm).
///
/// The single-letter names are Meeus's own, so the code can be read against the book.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub const fn pascha_julian(year: i64) -> Date {
    let a = year.rem_euclid(4);
    let b = year.rem_euclid(7);
    let c = year.rem_euclid(19);
    let d = (19 * c + 15).rem_euclid(30);
    let e = (2 * a + 4 * b - d + 34).rem_euclid(7);
    let f = d + e + 114;
    Date {
        year,
        month: f.div_euclid(31),
        day: f.rem_euclid(31) + 1,
    }
}

/// Julian Day Number of a Julian-calendar date (noon-based integer JDN).
#[must_use]
pub const fn julian_to_jdn(d: Date) -> i64 {
    let a = (14 - d.month).div_euclid(12);
    let y = d.year + 4800 - a;
    let m = d.month + 12 * a - 3;
    d.day + (153 * m + 2).div_euclid(5) + 365 * y + y.div_euclid(4) - 32083
}

/// Gregorian (civil) date of a Julian Day Number (Richards' algorithm).
///
/// The single-letter names are Richards' own, as published.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub const fn jdn_to_gregorian(j: i64) -> Date {
    let f = j + 1401 + ((4 * j + 274_277).div_euclid(146_097) * 3).div_euclid(4) - 38;
    let e = 4 * f + 3;
    let g = e.rem_euclid(1461).div_euclid(4);
    let h = 5 * g + 2;
    let day = h.rem_euclid(153).div_euclid(5) + 1;
    let month = (h.div_euclid(153) + 2).rem_euclid(12) + 1;
    let year = e.div_euclid(1461) - 4716 + (12 + 2 - month).div_euclid(12);
    Date { year, month, day }
}

/// Julian Day Number of Pascha in a year.
#[must_use]
pub const fn pascha_jdn(year: i64) -> i64 {
    julian_to_jdn(pascha_julian(year))
}

/// Day of the week of a JDN: 0 = Sunday ... 6 = Saturday.
#[must_use]
pub const fn weekday(jdn: i64) -> i64 {
    (jdn + 1).rem_euclid(7)
}

/// One feast of the movable cycle.
#[derive(Debug, Clone, Copy)]
pub struct Movable {
    /// Days from Pascha (negative before).
    pub offset: i64,
    /// Name as the Church's calendar gives it (orthocal.info's wording where it has one).
    pub name: &'static str,
}

/// The movable cycle, in order. Every offset was checked against orthocal.info's
/// calendar for 2026 (Pascha 12 April).
pub const MOVABLE: [Movable; 16] = [
    Movable {
        offset: -70,
        name: "Sunday of the Publican and the Pharisee",
    },
    Movable {
        offset: -63,
        name: "Sunday of the Prodigal Son",
    },
    Movable {
        offset: -56,
        name: "Sunday of Meatfare (the Last Judgment)",
    },
    Movable {
        offset: -49,
        name: "Sunday of Cheesefare: Expulsion of Adam from Paradise",
    },
    Movable {
        offset: -48,
        name: "Beginning of the Great Fast (Clean Monday)",
    },
    Movable {
        offset: -8,
        name: "Lazarus Saturday",
    },
    Movable {
        offset: -7,
        name: "Palm Sunday: Entrance of Our Lord into Jerusalem",
    },
    Movable {
        offset: -3,
        name: "Great and Holy Thursday",
    },
    Movable {
        offset: -2,
        name: "Great and Holy Friday",
    },
    Movable {
        offset: 0,
        name: "Holy Pascha",
    },
    Movable {
        offset: 7,
        name: "Antipascha: Sunday of Thomas",
    },
    Movable {
        offset: 24,
        name: "Midfeast of Pentecost",
    },
    Movable {
        offset: 39,
        name: "Ascension of the Lord",
    },
    Movable {
        offset: 49,
        name: "Pentecost: Feast of the Holy Trinity",
    },
    Movable {
        offset: 56,
        name: "Sunday of All Saints",
    },
    Movable {
        offset: 57,
        name: "The Apostles' Fast begins",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    const fn g(year: i64, month: i64, day: i64) -> Date {
        Date { year, month, day }
    }

    #[test]
    fn pascha_matches_orthocal_from_2026_to_4099() {
        // Read from orthocal.info's API on 2026-10-12 (date minus pascha_distance).
        let cases = [
            (2026, g(2026, 4, 12)),
            (2027, g(2027, 5, 2)),
            (2100, g(2100, 5, 2)),
            (2500, g(2500, 4, 25)),
            (3000, g(3000, 4, 20)),
            (4099, g(4099, 5, 3)),
        ];
        for (year, want) in cases {
            assert_eq!(jdn_to_gregorian(pascha_jdn(year)), want, "Pascha {year}");
        }
    }

    #[test]
    fn pascha_falls_on_a_sunday_in_every_year_to_200000() {
        let mut y = 1;
        while y <= 200_000 {
            assert_eq!(weekday(pascha_jdn(y)), 0, "Pascha {y} must be a Sunday");
            y += 1;
        }
    }

    #[test]
    fn the_julian_paschal_cycle_repeats_every_532_years() {
        for y in [1, 325, 2026, 40_000, 41_000] {
            let a = pascha_julian(y);
            let b = pascha_julian(y + 532);
            assert_eq!((a.month, a.day), (b.month, b.day));
        }
    }

    #[test]
    fn day_numbers_match_the_almanac_epochs() {
        // 2000 January 1 (Gregorian) is JDN 2451545; Julian 4713 BC Jan 1 (year -4712) is 0.
        assert_eq!(jdn_to_gregorian(2_451_545), g(2000, 1, 1));
        assert_eq!(julian_to_jdn(g(-4712, 1, 1)), 0);
        // The Gregorian reform: Julian 1582-10-04 was followed by Gregorian 1582-10-15.
        assert_eq!(
            jdn_to_gregorian(julian_to_jdn(g(1582, 10, 4)) + 1),
            g(1582, 10, 15)
        );
    }

    #[test]
    fn years_40000_and_41000_resolve_exactly() {
        // By hand from Meeus: 40000 -> a=0 b=2 c=5 d=20 e=1, f=135: April 12 (Julian).
        // 41000 -> a=0 b=1 c=17 d=8 e=2, f=124: April 1 (Julian).
        assert_eq!(pascha_julian(40_000), g(40_000, 4, 12));
        assert_eq!(pascha_julian(41_000), g(41_000, 4, 1));
        // The calendars drift 298 days apart by 40000 and 306 by 41000
        // (Y div 100 - Y div 400 - 2), so Pascha's civil date falls in February of the
        // following Gregorian year. Checked against an independent algorithm.
        assert_eq!(
            julian_to_jdn(g(40_000, 4, 12)) - gregorian_jdn(g(40_000, 4, 12)),
            298
        );
        assert_eq!(
            julian_to_jdn(g(41_000, 4, 1)) - gregorian_jdn(g(41_000, 4, 1)),
            306
        );
        assert_eq!(jdn_to_gregorian(pascha_jdn(40_000)), g(40_001, 2, 4));
        assert_eq!(jdn_to_gregorian(pascha_jdn(41_000)), g(41_001, 2, 1));
        assert_eq!(gregorian_jdn(g(40_001, 2, 4)), pascha_jdn(40_000));
        assert_eq!(weekday(pascha_jdn(40_000)), 0);
        assert_eq!(weekday(pascha_jdn(41_000)), 0);
    }

    const fn gregorian_jdn(d: Date) -> i64 {
        // Fliegel and Van Flandern, Gregorian calendar date to JDN, for the test only.
        let a = (14 - d.month).div_euclid(12);
        let y = d.year + 4800 - a;
        let m = d.month + 12 * a - 3;
        d.day + (153 * m + 2).div_euclid(5) + 365 * y + y.div_euclid(4) - y.div_euclid(100)
            + y.div_euclid(400)
            - 32045
    }

    #[test]
    fn gregorian_round_trip_holds_for_far_years() {
        for y in [1600, 2026, 40_000, 41_000, 100_000] {
            for (m, d) in [(1, 1), (2, 28), (3, 1), (12, 31)] {
                let date = g(y, m, d);
                assert_eq!(jdn_to_gregorian(gregorian_jdn(date)), date);
            }
        }
    }
}

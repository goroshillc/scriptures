//! The 66-book canon table: OSIS code, display name, URL slug, USFM code, testament.
//!
//! > *"Search the scriptures; for in them ye think ye have eternal life: and they are
//! > they which testify of me."* - John 5:39, KJV
//!
//! WHY one table: the original engine keys everything on OSIS (`John.3.16`), the
//! eBible red-letter source and orthocal speak USFM (`JHN`), and the public URLs need
//! readable slugs (`/john/3/`). Three vocabularies, one row per book, index-for-index
//! with `kjv.json` - verified by a test against the data, not assumed.

/// Which testament a book belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Testament {
    /// Genesis to Malachi.
    Old,
    /// Matthew to Revelation.
    New,
}

/// One book of the canon.
#[derive(Debug, Clone, Copy)]
pub struct BookInfo {
    /// OSIS code, the reader's join key (`"John"`).
    pub osis: &'static str,
    /// Display name exactly as `kjv.json` spells it (`"Song of Solomon"`).
    pub name: &'static str,
    /// URL path segment (`"song-of-solomon"`).
    pub slug: &'static str,
    /// USFM book code used by eBible.org and orthocal.info (`"SNG"`).
    pub usfm: &'static str,
    /// Old or New Testament.
    pub testament: Testament,
}

const fn ot(
    osis: &'static str,
    name: &'static str,
    slug: &'static str,
    usfm: &'static str,
) -> BookInfo {
    BookInfo {
        osis,
        name,
        slug,
        usfm,
        testament: Testament::Old,
    }
}

const fn nt(
    osis: &'static str,
    name: &'static str,
    slug: &'static str,
    usfm: &'static str,
) -> BookInfo {
    BookInfo {
        osis,
        name,
        slug,
        usfm,
        testament: Testament::New,
    }
}

/// The canon in `kjv.json` order (the same order as the original engine's `OSIS` array).
pub const BOOKS: [BookInfo; 66] = [
    ot("Gen", "Genesis", "genesis", "GEN"),
    ot("Exod", "Exodus", "exodus", "EXO"),
    ot("Lev", "Leviticus", "leviticus", "LEV"),
    ot("Num", "Numbers", "numbers", "NUM"),
    ot("Deut", "Deuteronomy", "deuteronomy", "DEU"),
    ot("Josh", "Joshua", "joshua", "JOS"),
    ot("Judg", "Judges", "judges", "JDG"),
    ot("Ruth", "Ruth", "ruth", "RUT"),
    ot("1Sam", "1 Samuel", "1-samuel", "1SA"),
    ot("2Sam", "2 Samuel", "2-samuel", "2SA"),
    ot("1Kgs", "1 Kings", "1-kings", "1KI"),
    ot("2Kgs", "2 Kings", "2-kings", "2KI"),
    ot("1Chr", "1 Chronicles", "1-chronicles", "1CH"),
    ot("2Chr", "2 Chronicles", "2-chronicles", "2CH"),
    ot("Ezra", "Ezra", "ezra", "EZR"),
    ot("Neh", "Nehemiah", "nehemiah", "NEH"),
    ot("Esth", "Esther", "esther", "EST"),
    ot("Job", "Job", "job", "JOB"),
    ot("Ps", "Psalms", "psalms", "PSA"),
    ot("Prov", "Proverbs", "proverbs", "PRO"),
    ot("Eccl", "Ecclesiastes", "ecclesiastes", "ECC"),
    ot("Song", "Song of Solomon", "song-of-solomon", "SNG"),
    ot("Isa", "Isaiah", "isaiah", "ISA"),
    ot("Jer", "Jeremiah", "jeremiah", "JER"),
    ot("Lam", "Lamentations", "lamentations", "LAM"),
    ot("Ezek", "Ezekiel", "ezekiel", "EZK"),
    ot("Dan", "Daniel", "daniel", "DAN"),
    ot("Hos", "Hosea", "hosea", "HOS"),
    ot("Joel", "Joel", "joel", "JOL"),
    ot("Amos", "Amos", "amos", "AMO"),
    ot("Obad", "Obadiah", "obadiah", "OBA"),
    ot("Jonah", "Jonah", "jonah", "JON"),
    ot("Mic", "Micah", "micah", "MIC"),
    ot("Nah", "Nahum", "nahum", "NAM"),
    ot("Hab", "Habakkuk", "habakkuk", "HAB"),
    ot("Zeph", "Zephaniah", "zephaniah", "ZEP"),
    ot("Hag", "Haggai", "haggai", "HAG"),
    ot("Zech", "Zechariah", "zechariah", "ZEC"),
    ot("Mal", "Malachi", "malachi", "MAL"),
    nt("Matt", "Matthew", "matthew", "MAT"),
    nt("Mark", "Mark", "mark", "MRK"),
    nt("Luke", "Luke", "luke", "LUK"),
    nt("John", "John", "john", "JHN"),
    nt("Acts", "Acts", "acts", "ACT"),
    nt("Rom", "Romans", "romans", "ROM"),
    nt("1Cor", "1 Corinthians", "1-corinthians", "1CO"),
    nt("2Cor", "2 Corinthians", "2-corinthians", "2CO"),
    nt("Gal", "Galatians", "galatians", "GAL"),
    nt("Eph", "Ephesians", "ephesians", "EPH"),
    nt("Phil", "Philippians", "philippians", "PHP"),
    nt("Col", "Colossians", "colossians", "COL"),
    nt("1Thess", "1 Thessalonians", "1-thessalonians", "1TH"),
    nt("2Thess", "2 Thessalonians", "2-thessalonians", "2TH"),
    nt("1Tim", "1 Timothy", "1-timothy", "1TI"),
    nt("2Tim", "2 Timothy", "2-timothy", "2TI"),
    nt("Titus", "Titus", "titus", "TIT"),
    nt("Phlm", "Philemon", "philemon", "PHM"),
    nt("Heb", "Hebrews", "hebrews", "HEB"),
    nt("Jas", "James", "james", "JAS"),
    nt("1Pet", "1 Peter", "1-peter", "1PE"),
    nt("2Pet", "2 Peter", "2-peter", "2PE"),
    nt("1John", "1 John", "1-john", "1JN"),
    nt("2John", "2 John", "2-john", "2JN"),
    nt("3John", "3 John", "3-john", "3JN"),
    nt("Jude", "Jude", "jude", "JUD"),
    nt("Rev", "Revelation", "revelation", "REV"),
];

/// Index of a book by OSIS code (`"John"` -> 42).
#[must_use]
pub fn index_of_osis(osis: &str) -> Option<usize> {
    BOOKS.iter().position(|b| b.osis == osis)
}

/// Index of a book by USFM code (`"JHN"` -> 42).
#[must_use]
pub fn index_of_usfm(usfm: &str) -> Option<usize> {
    BOOKS.iter().position(|b| b.usfm == usfm)
}

/// The book at an index, if the index is inside the canon.
#[must_use]
pub fn book(index: usize) -> Option<&'static BookInfo> {
    BOOKS.get(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn john_is_book_forty_three_in_every_vocabulary() {
        assert_eq!(index_of_osis("John"), Some(42));
        assert_eq!(index_of_usfm("JHN"), Some(42));
        assert_eq!(book(42).map(|b| b.slug), Some("john"));
    }

    #[test]
    fn slugs_and_codes_are_unique() {
        for (i, a) in BOOKS.iter().enumerate() {
            for b in BOOKS.iter().skip(i + 1) {
                assert_ne!(a.slug, b.slug);
                assert_ne!(a.osis, b.osis);
                assert_ne!(a.usfm, b.usfm);
            }
        }
    }
}

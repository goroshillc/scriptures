//! Book names, common abbreviations and reference parsing ("John 3:16", "Jn 3", "Ps 23").
//!
//! WHY its own table: the search box must take a reader straight to a passage the way a
//! printed concordance would, in the browser, without the builder's canon module. The
//! names and chapter counts are the King James Version's; the builder's tests hold this
//! table to the on-disk KJV book for book, so the two can never drift apart.

/// One book: display name exactly as the KJV data spells it, URL slug, chapter count, and
/// the abbreviations a reader is likely to type (lowercase, without spaces or dots).
pub struct Book {
    /// Display name (`"Song of Solomon"`).
    pub name: &'static str,
    /// URL path segment (`"song-of-solomon"`).
    pub slug: &'static str,
    /// Chapters in the King James Version.
    pub chapters: u16,
    /// Accepted abbreviations, folded (`"jn"`, `"jhn"`).
    pub aliases: &'static [&'static str],
}

const fn b(
    name: &'static str,
    slug: &'static str,
    chapters: u16,
    aliases: &'static [&'static str],
) -> Book {
    Book {
        name,
        slug,
        chapters,
        aliases,
    }
}

/// The 66 books in canonical order, Genesis to Revelation.
pub const BOOKS: [Book; 66] = [
    b("Genesis", "genesis", 50, &["gen", "ge", "gn"]),
    b("Exodus", "exodus", 40, &["exod", "exo", "ex"]),
    b("Leviticus", "leviticus", 27, &["lev", "le", "lv"]),
    b("Numbers", "numbers", 36, &["num", "nu", "nm", "nb"]),
    b(
        "Deuteronomy",
        "deuteronomy",
        34,
        &["deut", "deu", "de", "dt"],
    ),
    b("Joshua", "joshua", 24, &["josh", "jos", "jsh"]),
    b("Judges", "judges", 21, &["judg", "jdg", "jg", "jdgs"]),
    b("Ruth", "ruth", 4, &["ruth", "rth", "ru"]),
    b("1 Samuel", "1-samuel", 31, &["1sam", "1sa", "1sm", "1s"]),
    b("2 Samuel", "2-samuel", 24, &["2sam", "2sa", "2sm", "2s"]),
    b("1 Kings", "1-kings", 22, &["1kgs", "1ki", "1kin", "1k"]),
    b("2 Kings", "2-kings", 25, &["2kgs", "2ki", "2kin", "2k"]),
    b(
        "1 Chronicles",
        "1-chronicles",
        29,
        &["1chr", "1ch", "1chron"],
    ),
    b(
        "2 Chronicles",
        "2-chronicles",
        36,
        &["2chr", "2ch", "2chron"],
    ),
    b("Ezra", "ezra", 10, &["ezra", "ezr"]),
    b("Nehemiah", "nehemiah", 13, &["neh", "ne"]),
    b("Esther", "esther", 10, &["esth", "est", "es"]),
    b("Job", "job", 42, &["job", "jb"]),
    b(
        "Psalms",
        "psalms",
        150,
        &["ps", "psa", "psalm", "pss", "psm", "pslm"],
    ),
    b("Proverbs", "proverbs", 31, &["prov", "pro", "prv", "pr"]),
    b(
        "Ecclesiastes",
        "ecclesiastes",
        12,
        &["eccl", "ecc", "eccles", "ec", "qoh"],
    ),
    b(
        "Song of Solomon",
        "song-of-solomon",
        8,
        &[
            "song",
            "sos",
            "so",
            "songofsongs",
            "canticles",
            "cant",
            "sg",
        ],
    ),
    b("Isaiah", "isaiah", 66, &["isa", "is"]),
    b("Jeremiah", "jeremiah", 52, &["jer", "je", "jr"]),
    b("Lamentations", "lamentations", 5, &["lam", "la"]),
    b("Ezekiel", "ezekiel", 48, &["ezek", "eze", "ezk"]),
    b("Daniel", "daniel", 12, &["dan", "da", "dn"]),
    b("Hosea", "hosea", 14, &["hos", "ho"]),
    b("Joel", "joel", 3, &["joel", "jl"]),
    b("Amos", "amos", 9, &["amos", "am"]),
    b("Obadiah", "obadiah", 1, &["obad", "ob"]),
    b("Jonah", "jonah", 4, &["jonah", "jon", "jnh"]),
    b("Micah", "micah", 7, &["mic", "mc"]),
    b("Nahum", "nahum", 3, &["nah", "na"]),
    b("Habakkuk", "habakkuk", 3, &["hab", "hb"]),
    b("Zephaniah", "zephaniah", 3, &["zeph", "zep", "zp"]),
    b("Haggai", "haggai", 2, &["hag", "hg"]),
    b("Zechariah", "zechariah", 14, &["zech", "zec", "zc"]),
    b("Malachi", "malachi", 4, &["mal", "ml"]),
    b("Matthew", "matthew", 28, &["matt", "mat", "mt"]),
    b("Mark", "mark", 16, &["mark", "mrk", "mk", "mr"]),
    b("Luke", "luke", 24, &["luke", "luk", "lk"]),
    b("John", "john", 21, &["john", "jn", "jhn", "joh"]),
    b("Acts", "acts", 28, &["acts", "act", "ac"]),
    b("Romans", "romans", 16, &["rom", "ro", "rm"]),
    b("1 Corinthians", "1-corinthians", 16, &["1cor", "1co"]),
    b("2 Corinthians", "2-corinthians", 13, &["2cor", "2co"]),
    b("Galatians", "galatians", 6, &["gal", "ga"]),
    b("Ephesians", "ephesians", 6, &["eph", "ephes"]),
    b("Philippians", "philippians", 4, &["phil", "php", "pp"]),
    b("Colossians", "colossians", 4, &["col"]),
    b(
        "1 Thessalonians",
        "1-thessalonians",
        5,
        &["1thess", "1thes", "1th"],
    ),
    b(
        "2 Thessalonians",
        "2-thessalonians",
        3,
        &["2thess", "2thes", "2th"],
    ),
    b("1 Timothy", "1-timothy", 6, &["1tim", "1ti", "1tm"]),
    b("2 Timothy", "2-timothy", 4, &["2tim", "2ti", "2tm"]),
    b("Titus", "titus", 3, &["titus", "tit"]),
    b("Philemon", "philemon", 1, &["phlm", "philem", "phm"]),
    b("Hebrews", "hebrews", 13, &["heb"]),
    b("James", "james", 5, &["jas", "jam", "jm"]),
    b("1 Peter", "1-peter", 5, &["1pet", "1pe", "1pt", "1p"]),
    b("2 Peter", "2-peter", 3, &["2pet", "2pe", "2pt", "2p"]),
    b(
        "1 John",
        "1-john",
        5,
        &["1john", "1jn", "1jo", "1jhn", "1j"],
    ),
    b(
        "2 John",
        "2-john",
        1,
        &["2john", "2jn", "2jo", "2jhn", "2j"],
    ),
    b(
        "3 John",
        "3-john",
        1,
        &["3john", "3jn", "3jo", "3jhn", "3j"],
    ),
    b("Jude", "jude", 1, &["jude", "jud", "jd"]),
    b(
        "Revelation",
        "revelation",
        22,
        &["rev", "re", "rv", "revelations", "apocalypse"],
    ),
];

/// A parsed reference. Chapter and verses are 1-based; an absent verse means the
/// whole chapter, and an absent chapter means the whole book.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reference {
    /// Index into [`BOOKS`].
    pub book: usize,
    /// Chapter, when one was given.
    pub chapter: Option<u16>,
    /// First verse, when one was given.
    pub verse: Option<u16>,
    /// Last verse of a range ("John 3:16-18"), when one was given.
    pub verse_end: Option<u16>,
}

/// Lowercase, no spaces, dots or apostrophes: the form aliases are stored in.
fn squash(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '.' | '\''))
        .flat_map(char::to_lowercase)
        .collect()
}

/// "1", "i", "1st", "first" -> '1' (and the same for 2 and 3).
fn ordinal(token: &str) -> Option<char> {
    match token {
        "1" | "i" | "1st" | "first" => Some('1'),
        "2" | "ii" | "2nd" | "second" => Some('2'),
        "3" | "iii" | "3rd" | "third" => Some('3'),
        _ => None,
    }
}

/// The book a typed name means, or `None` when it names no book or more than one.
///
/// Exact names and listed abbreviations win; otherwise a unique prefix of at least three
/// letters of one full name is accepted ("phili" is refused: Philippians or Philemon).
#[must_use]
pub fn book_index(typed: &str) -> Option<usize> {
    let tokens: Vec<String> = typed
        .split_whitespace()
        .map(|t| t.trim_matches('.').to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    let (prefix, rest) = match tokens.as_slice() {
        [first, rest @ ..] if !rest.is_empty() => {
            ordinal(first).map_or((None, tokens.as_slice()), |digit| (Some(digit), rest))
        }
        _ => (None, tokens.as_slice()),
    };
    let mut key: String = prefix.map(String::from).unwrap_or_default();
    key.push_str(&squash(&rest.concat()));
    if key.is_empty() {
        return None;
    }
    if let Some(found) = BOOKS
        .iter()
        .position(|book| squash(book.name) == key || book.aliases.contains(&key.as_str()))
    {
        return Some(found);
    }
    if key.chars().filter(char::is_ascii_alphabetic).count() < 3 {
        return None;
    }
    let mut matches = BOOKS
        .iter()
        .enumerate()
        .filter(|(_, book)| squash(book.name).starts_with(&key));
    match (matches.next(), matches.next()) {
        (Some((index, _)), None) => Some(index),
        _ => None,
    }
}

fn number(value: &str) -> Option<u16> {
    if value.is_empty() || value.len() > 4 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok().filter(|n| *n > 0)
}

/// Parses a reference as people type it.
///
/// "John 3:16", "Jn 3", "Ps 23", "1 Cor 13:4-7", "Jude 3", "Song of Solomon 2:1" and
/// "Genesis" are all understood. Returns `None` for anything that is not plainly a
/// reference, so ordinary search words are never taken for one.
#[must_use]
pub fn parse(input: &str) -> Option<Reference> {
    let text = input.trim();
    if text.is_empty() || text.len() > 64 {
        return None;
    }
    // Split the trailing numeric part ("3:16-18", "3 16", "23") from the book name.
    let normalized: String = text
        .chars()
        .map(|c| match c {
            '\u{2013}' | '\u{2014}' => '-',
            '.' => ':',
            other => other,
        })
        .collect();
    let digits_start = normalized
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit() || matches!(c, ':' | '-' | ' '))
        .last()
        .map_or(normalized.len(), |(i, _)| i);
    let (mut name, mut tail) = normalized.split_at(digits_start);
    // A leading book ordinal ("1 John 3") is part of the name, not the numbers.
    if name.trim().is_empty() {
        let trimmed = tail.trim_start();
        let first_space = trimmed.find(' ')?;
        let (ordinal_part, after) = trimmed.split_at(first_space);
        let after = after.trim_start();
        let next_digit = after
            .char_indices()
            .find(|(_, c)| c.is_ascii_digit())
            .map_or(after.len(), |(i, _)| i);
        let (word, numbers) = after.split_at(next_digit);
        if word.trim().is_empty() {
            return None;
        }
        let joined = format!("{ordinal_part} {}", word.trim());
        let index = book_index(&joined)?;
        return finish(index, numbers.trim());
    }
    name = name.trim_end_matches([':', ' ']);
    tail = tail.trim();
    let index = book_index(name.trim_end_matches('.'))?;
    finish(index, tail)
}

fn finish(index: usize, numbers: &str) -> Option<Reference> {
    let book = BOOKS.get(index)?;
    if numbers.is_empty() {
        return Some(Reference {
            book: index,
            chapter: None,
            verse: None,
            verse_end: None,
        });
    }
    let parts: Vec<&str> = numbers
        .split([':', ' '])
        .filter(|p| !p.is_empty())
        .collect();
    let (chapter, verses) = match parts.as_slice() {
        [single] if book.chapters == 1 => ("1", Some(*single)),
        [single] => (*single, None),
        [chapter, verses] => (*chapter, Some(*verses)),
        _ => return None,
    };
    // "Gen 1-3": a chapter range opens its first chapter.
    let chapter = number(chapter.split('-').next().unwrap_or(""))?;
    if chapter > book.chapters {
        return None;
    }
    let (verse, verse_end) = match verses {
        None => (None, None),
        Some(range) => {
            let mut ends = range.split('-');
            let first = number(ends.next().unwrap_or(""))?;
            let last = match ends.next() {
                Some(value) => Some(number(value)?).filter(|end| *end > first),
                None => None,
            };
            if ends.next().is_some() {
                return None;
            }
            (Some(first), last)
        }
    };
    Some(Reference {
        book: index,
        chapter: Some(chapter),
        verse,
        verse_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::unnecessary_wraps)]
    fn at(book: usize, chapter: u16, verse: Option<u16>) -> Option<Reference> {
        Some(Reference {
            book,
            chapter: Some(chapter),
            verse,
            verse_end: None,
        })
    }

    #[test]
    fn the_common_ways_of_writing_a_reference_reach_the_passage() {
        assert_eq!(parse("John 3:16"), at(42, 3, Some(16)));
        assert_eq!(parse("jn 3"), at(42, 3, None));
        assert_eq!(parse("Ps 23"), at(18, 23, None));
        assert_eq!(parse("Psalm 119:105"), at(18, 119, Some(105)));
        assert_eq!(parse("1 Cor 13"), at(45, 13, None));
        assert_eq!(parse("I Corinthians 13:4"), at(45, 13, Some(4)));
        assert_eq!(parse("second kings 2"), at(11, 2, None));
        assert_eq!(parse("Song of Solomon 2:1"), at(21, 2, Some(1)));
        assert_eq!(parse("Gen. 1.1"), at(0, 1, Some(1)));
        assert_eq!(parse("Rev 22:21"), at(65, 22, Some(21)));
        assert_eq!(parse("1 John 4:8"), at(61, 4, Some(8)));
        assert_eq!(parse("1jn 4 8"), at(61, 4, Some(8)));
        assert_eq!(
            parse("John 3:16-18"),
            Some(Reference {
                book: 42,
                chapter: Some(3),
                verse: Some(16),
                verse_end: Some(18),
            })
        );
    }

    #[test]
    fn single_chapter_books_read_one_number_as_a_verse() {
        assert_eq!(parse("Jude 3"), at(64, 1, Some(3)));
        assert_eq!(parse("Philemon 1:4"), at(56, 1, Some(4)));
    }

    #[test]
    fn a_book_alone_names_the_book_and_ordinary_words_are_not_references() {
        assert_eq!(
            parse("Genesis"),
            Some(Reference {
                book: 0,
                chapter: None,
                verse: None,
                verse_end: None,
            })
        );
        assert_eq!(parse("love one another"), None);
        assert_eq!(parse("phi 2"), None, "Philippians or Philemon");
        assert_eq!(parse("phili 2"), at(49, 2, None));
        assert_eq!(parse("John 22"), None);
        assert_eq!(parse("Ps 151"), None);
        assert_eq!(parse("John 0:1"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn every_book_is_found_by_its_full_name_and_every_alias() {
        for (index, book) in BOOKS.iter().enumerate() {
            assert_eq!(book_index(book.name), Some(index), "{}", book.name);
            for alias in book.aliases {
                assert_eq!(book_index(alias), Some(index), "{alias}");
            }
        }
        let total: u32 = BOOKS.iter().map(|book| u32::from(book.chapters)).sum();
        assert_eq!(total, 1189);
    }
}

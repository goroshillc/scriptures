//! Every verse inscribed in this crate's source is verbatim from the on-disk KJV.
//!
//! The rule: a red-letter inscription is never paraphrased, shortened or adapted;
//! a misquoted verse is worse than a missing one. This test reads every
//! `*"…"* - Book C:V, KJV` quotation in `src/` and compares it with `data/kjv.json`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use goroshi_scriptures::bible::Bible;
use std::path::Path;

fn doc_text(src: &str) -> String {
    src.lines()
        .map(str::trim_start)
        .filter_map(|l| l.strip_prefix("//! >").or_else(|| l.strip_prefix("// >")))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ")
}

fn quotes(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("*\"") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("\"*") else { break };
        let verse = after[..end].to_string();
        let tail = &after[end + 2..];
        let Some(dash) = tail.find(" - ") else { break };
        let Some(kjv) = tail.find(", KJV") else { break };
        out.push((verse, tail[dash + 3..kjv].trim().to_string()));
        rest = &tail[kjv..];
    }
    out
}

#[test]
fn every_inscribed_verse_is_verbatim_from_the_kjv_on_disk() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bible = Bible::load(&root.join("data/kjv.json")).expect("kjv loads");
    let mut checked = 0usize;
    let dirs = [root.join("src"), root.join("computus/src")];
    for entry in dirs
        .iter()
        .flat_map(|d| std::fs::read_dir(d).expect("src dir"))
    {
        let path = entry.expect("entry").path();
        let src = std::fs::read_to_string(&path).expect("source readable");
        for (verse, reference) in quotes(&doc_text(&src)) {
            let (book, cv) = reference.rsplit_once(' ').expect("Book C:V");
            let (c, v) = cv.split_once(':').expect("C:V");
            let bi = (0..66)
                .find(|&i| bible.book_name(i) == Some(book))
                .unwrap_or_else(|| panic!("{} names unknown book {book}", path.display()));
            let disk = bible
                .verse_text(bi, c.parse().expect("chapter"), v.parse().expect("verse"))
                .expect("verse on disk");
            assert_eq!(
                verse,
                disk,
                "{}: {reference} must be verbatim",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked >= 16, "only {checked} inscriptions found");
}

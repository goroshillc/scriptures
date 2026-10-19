//! The plates in the order of the Bible itself, and the gallery that walks through them.
//!
//! > *"In the beginning God created the heaven and the earth."* - Genesis 1:1, KJV
//!
//! WHY one ordering function: the Doré plates belong in the Bible's own order: the gallery, the plate viewer's previous and next, and the chapter pages
//! must all follow Genesis 1 to Revelation 22, then chapter and verse. Plates from the
//! books the Orthodox Church reads with the Old Testament beyond the 66 (Tobit, Baruch,
//! Susanna, Bel and the Dragon, the Maccabees) take the places the Greek Bible gives them,
//! so every plate has exactly one place and none is left out.

use crate::art::Artwork;
use crate::canon;
use crate::html::esc;
use crate::render::Ctx;
use std::fmt::Write as _;

/// Where a plate stands in the reading order, and the book it is filed under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// (book position, sub-book, chapter, verse): sorts Genesis 1:1 to Revelation 22:21.
    pub key: (usize, usize, usize, usize),
    /// The book's display name.
    pub book: String,
    /// The book's slug, for the gallery's filter.
    pub slug: String,
}

/// The Greek Bible's places for the books beyond the 66: (name, anchor book index, sub,
/// chapter offset). Tobit follows Nehemiah; the Maccabees follow Esther; Baruch follows
/// Jeremiah; Susanna and Bel are Daniel 13 and 14.
const BEYOND: [(&str, usize, usize, usize); 6] = [
    ("Tobit", 15, 1, 0),
    ("1 Maccabees", 16, 1, 0),
    ("2 Maccabees", 16, 2, 0),
    ("Baruch", 23, 1, 0),
    ("Susanna", 26, 0, 13),
    ("Bel and the Dragon", 26, 0, 14),
];

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// The first chapter and verse of a reference such as `"Tobit 6:1-18"` or
/// `"Susanna 36-64 (Daniel 13 in the Greek Bible)"` (a single-chapter book's numbers are
/// verses).
fn first_numbers(rest: &str) -> (usize, usize) {
    let head = rest.split(['(', ';']).next().unwrap_or("").trim();
    let first = head.split('-').next().unwrap_or("").trim();
    let mut parts = first
        .split(':')
        .map(|p| p.trim().parse::<usize>().unwrap_or(0));
    match (parts.next(), parts.next()) {
        (Some(c), Some(v)) => (c, v),
        (Some(v), None) => (0, v),
        _ => (0, 0),
    }
}

/// A plate's place, or `None` when it names no book this site knows (a test keeps
/// that from ever happening to an admitted plate).
#[must_use]
pub fn place(w: &Artwork) -> Option<Place> {
    if let Some(start) = w.start() {
        let info = canon::book(start.book)?;
        return Some(Place {
            key: (start.book, 0, start.chapter, start.verse),
            book: info.name.to_string(),
            slug: info.slug.to_string(),
        });
    }
    let other = w.other_ref.as_deref()?;
    let (name, anchor, sub, chapter_offset) = BEYOND
        .iter()
        .find(|(name, ..)| other.starts_with(name))
        .copied()?;
    let (chapter, verse) = first_numbers(other.get(name.len()..).unwrap_or(""));
    let chapter = if chapter_offset > 0 {
        chapter_offset
    } else {
        chapter
    };
    Some(Place {
        key: (anchor, sub, chapter, verse),
        book: name.to_string(),
        slug: slugify(name),
    })
}

/// Every placed plate in canonical order (ties keep the plan's order).
#[must_use]
pub fn ordered(works: &[Artwork]) -> Vec<(&Artwork, Place)> {
    let mut placed: Vec<(usize, &Artwork, Place)> = works
        .iter()
        .enumerate()
        .filter_map(|(i, w)| place(w).map(|p| (i, w, p)))
        .collect();
    placed.sort_by(|a, b| a.2.key.cmp(&b.2.key).then(a.0.cmp(&b.0)));
    placed.into_iter().map(|(_, w, p)| (w, p)).collect()
}

/// The previous and next plate of `index` in a loop: the first plate's previous is the
/// last, and the last plate's next is the first, as the reading itself loops.
#[must_use]
pub fn neighbours<T>(list: &[T], index: usize) -> (Option<&T>, Option<&T>) {
    if list.len() < 2 {
        return (None, None);
    }
    let prev = index.checked_sub(1).unwrap_or(list.len() - 1);
    let next = if index + 1 >= list.len() {
        0
    } else {
        index + 1
    };
    (list.get(prev), list.get(next))
}

/// The gallery page: every plate in canonical order, filed by book, with a filter.
#[must_use]
pub fn page(ctx: &Ctx<'_>, path: &str) -> String {
    let order = ordered(&ctx.art.works);
    let mut books: Vec<(String, String, usize)> = Vec::new();
    for (_, p) in &order {
        match books.last_mut() {
            Some((slug, _, n)) if *slug == p.slug => *n += 1,
            _ => books.push((p.slug.clone(), p.book.clone(), 1)),
        }
    }
    let mut b = String::from("<div class=\"wrap wide\">");
    let _ = write!(
        b,
        "<nav class=\"crumbs\" aria-label=\"Breadcrumb\"><ol><li><a href=\"{}\">The Scriptures</a></li><li aria-current=\"page\">Dor\u{e9} and the masters</li></ol></nav>",
        esc(&ctx.cfg.href(""))
    );
    let _ = write!(
        b,
        "<h1>The plates, in the order of the Bible</h1><p class=\"sub\">{} public-domain works by Gustave Dor\u{e9}, Albrecht D\u{fc}rer and Titian, from Genesis to Revelation, each beside its passage. Choose a plate to see it whole.</p>",
        order.len()
    );
    b.push_str("<form class=\"gallery-tools\" id=\"gallery-filter\" action=\"#\"><label>Book<select id=\"gallery-book\" name=\"book\"><option value=\"\">Every book</option>");
    for (slug, name, n) in &books {
        let _ = write!(
            b,
            "<option value=\"{}\">{} ({n})</option>",
            esc(slug),
            esc(name)
        );
    }
    let _ = write!(
        b,
        "</select></label><p class=\"meta\" id=\"gallery-count\" role=\"status\">{} plates</p><p class=\"meta\"><a href=\"{}?kind=art\">Search the art</a></p></form>",
        order.len(),
        esc(&ctx.cfg.href("search/"))
    );
    let mut current = String::new();
    for (w, p) in &order {
        if p.slug != current {
            if !current.is_empty() {
                b.push_str("</ul></section>");
            }
            let _ = write!(
                b,
                "<section class=\"gallery-book\" id=\"art-{}\" data-book=\"{}\"><h2>{}</h2><ul class=\"gallery\">",
                esc(&p.slug),
                esc(&p.slug),
                esc(&p.book)
            );
            current.clone_from(&p.slug);
        }
        b.push_str(&ctx.tile_pub(w));
    }
    if !current.is_empty() {
        b.push_str("</ul></section>");
    }
    b.push_str("</div>");
    let meta = crate::html::PageMeta {
        title: "Dor\u{e9}'s Bible Plates in Canonical Order \u{b7} The Scriptures".to_string(),
        description: format!(
            "{} public-domain Bible plates by Gustave Dor\u{e9} and the masters, in the order of the Bible from Genesis to Revelation, each at its King James passage.",
            order.len()
        ),
        path: path.to_string(),
        og: order.first().and_then(|(w, _)| ctx.og_pub(w)),
        og_type: "website",
        section: "art",
        ..crate::html::PageMeta::default()
    };
    let mut html = crate::html::page(ctx.cfg, &meta, &b);
    if path != "art/" {
        // The preserved address shows the same gallery; the search engines are told
        // which address is the gallery's own.
        html = html.replace(
            &format!(
                "<link rel=\"canonical\" href=\"{}\">",
                esc(&ctx.cfg.abs(path))
            ),
            &format!(
                "<link rel=\"canonical\" href=\"{}\">",
                esc(&ctx.cfg.abs("art/"))
            ),
        );
    }
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::Manifest;

    fn manifest() -> Option<Manifest> {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let mut m = Manifest::load(&data.join("art_manifest.json")).ok()?;
        m.apply(&crate::art::Exclusions::load(&data.join("ag_child_removed.tsv")).ok()?);
        m.apply_review(&crate::art::Review::load(&data.join("content_review.tsv")).ok()?)
            .ok()?;
        Some(m)
    }

    #[test]
    fn books_beyond_the_66_take_their_greek_bible_places() {
        assert_eq!(first_numbers(" 6:1-18"), (6, 1));
        assert_eq!(first_numbers(" 1:1; 2:1-23"), (1, 1));
        assert_eq!(
            first_numbers(" 36-64 (Daniel 13 in the Greek Bible)"),
            (0, 36)
        );
        assert_eq!(slugify("Bel and the Dragon"), "bel-and-the-dragon");
    }

    #[test]
    fn neighbours_loop_at_both_ends() {
        let list = [1, 2, 3];
        assert_eq!(neighbours(&list, 0), (Some(&3), Some(&2)));
        assert_eq!(neighbours(&list, 2), (Some(&2), Some(&1)));
        assert_eq!(neighbours(&[1], 0), (None, None));
    }

    /// Every admitted plate is placed exactly once, and the gallery order is the
    /// canonical order: never decreasing in (book, chapter, verse).
    #[test]
    fn the_gallery_follows_the_bible_and_places_every_plate_once() {
        let Some(m) = manifest() else {
            // The data directory is outside the crate; the build checks it as well.
            return;
        };
        let order = ordered(&m.works);
        assert_eq!(order.len(), m.works.len(), "no plate is unplaced");
        let mut slugs: Vec<&str> = order.iter().map(|(w, _)| w.slug.as_str()).collect();
        assert!(order.windows(2).all(|p| match p {
            [a, b] => a.1.key <= b.1.key,
            _ => true,
        }));
        slugs.sort_unstable();
        let before = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), before, "no plate is duplicated");
        assert_eq!(
            order.first().map(|(w, _)| w.slug.as_str()),
            Some("the-creation-of-light")
        );
    }
}

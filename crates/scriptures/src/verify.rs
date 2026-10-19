//! Crawls the built site from its home page: every internal link and image must
//! resolve, and every chapter of the KJV must be reachable.
//!
//! > *"Therefore whosoever heareth these sayings of mine, and doeth them, I will liken him
//! > unto a wise man, which built his house upon a rock:"* - Matthew 7:24, KJV
//!
//! WHY: the full KJV must be readable on the site. "Every chapter is
//! there" is proven by following the links a reader would follow, not by counting files.

use crate::canon;
use crate::{Error, Result};
use std::collections::{BTreeSet, VecDeque};
use std::path::Path;

/// What the crawl found.
#[derive(Debug, Default)]
pub struct Crawl {
    /// HTML pages reached from the home page.
    pub pages: usize,
    /// Distinct chapter pages reached.
    pub chapters: usize,
    /// Distinct book pages reached.
    pub books: usize,
    /// Images referenced and present.
    pub images: usize,
    /// (page, target) pairs that point at nothing.
    pub broken: Vec<(String, String)>,
    /// Pages checked for the light default and the shared design tokens.
    pub themed: usize,
    /// Pages that would open dark, or lack the shared tokens: (page, reason).
    pub theme_failures: Vec<(String, String)>,
    /// Whether "next" on Revelation 22 is Genesis 1 and "previous" on Genesis 1 is
    /// Revelation 22.
    pub loop_ok: bool,
}

fn attr_values<'a>(html: &'a str, attr: &str) -> Vec<&'a str> {
    let key = format!("{attr}=\"");
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find(&key) {
        let after = rest.get(i + key.len()..).unwrap_or("");
        let end = after.find('"').unwrap_or(after.len());
        out.push(after.get(..end).unwrap_or(""));
        rest = after.get(end..).unwrap_or("");
    }
    out
}

/// Every URL in the `srcset` and `imagesrcset` attributes ("a.jpg 480w, b.jpg 720w").
fn srcset_urls(html: &str) -> Vec<&str> {
    attr_values(html, "srcset")
        .into_iter()
        .flat_map(|set| set.split(','))
        .filter_map(|candidate| candidate.split_whitespace().next())
        .collect()
}

/// Why a page would not open in the light default with the shared design, if it would not.
///
/// The reader opens light for everyone with no saved choice: the root element carries no
/// theme, the colour scheme is declared light, the shared tokens (white paper) are inline,
/// and the one theme script turns dark on only for a saved choice of dark.
#[must_use]
pub fn theme_problem(html: &str) -> Option<&'static str> {
    let head_end = html.find("</head>")?;
    let head = html.get(..head_end).unwrap_or("");
    let start = html.find("<html")?;
    let html_tag = html
        .get(start..)
        .and_then(|rest| rest.get(..=rest.find('>')?))
        .unwrap_or("");
    if html_tag.contains("data-theme") || html_tag.contains("class=\"dark") {
        return Some("the root element sets a theme");
    }
    if !head.contains("<meta name=\"color-scheme\" content=\"light\">") {
        return Some("the colour scheme is not declared light");
    }
    if !head.contains("--paper: #ffffff") || !head.contains(":root[data-theme=\"dark\"]") {
        return Some("the shared design tokens are missing");
    }
    if head.contains("prefers-color-scheme") {
        return Some("the page follows the system theme instead of the reader's choice");
    }
    None
}

fn is_chapter(rel: &str) -> bool {
    let parts: Vec<&str> = rel
        .trim_end_matches("index.html")
        .trim_end_matches('/')
        .split('/')
        .collect();
    matches!(parts.as_slice(), [slug, n] if canon::BOOKS.iter().any(|b| b.slug == *slug) && n.chars().all(|c| c.is_ascii_digit()))
}

fn is_book(rel: &str) -> bool {
    let parts: Vec<&str> = rel
        .trim_end_matches("index.html")
        .trim_end_matches('/')
        .split('/')
        .collect();
    matches!(parts.as_slice(), [slug] if canon::BOOKS.iter().any(|b| b.slug == *slug))
}

/// Crawls `dist`, whose pages link under `base_path` (`/` or `/scriptures/`).
///
/// # Errors
/// Fails when a reached page cannot be read.
pub fn crawl(dist: &Path, base_path: &str) -> Result<Crawl> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut images: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = VecDeque::from(["index.html".to_string()]);
    seen.insert("index.html".to_string());
    let mut c = Crawl::default();
    while let Some(rel) = queue.pop_front() {
        let path = dist.join(&rel);
        let html = std::fs::read_to_string(&path).map_err(|e| Error::io("read", &path, e))?;
        c.pages += 1;
        for href in attr_values(&html, "href")
            .into_iter()
            .chain(attr_values(&html, "src"))
            .chain(srcset_urls(&html))
            .chain(attr_values(&html, "data-avif"))
            .chain(attr_values(&html, "data-webp"))
        {
            let target = href.split(['#', '?']).next().unwrap_or("");
            let Some(inside) = target.strip_prefix(base_path) else {
                continue;
            };
            if target.is_empty() || target.contains("://") {
                continue;
            }
            let file = if inside.is_empty() || inside.ends_with('/') {
                format!("{inside}index.html")
            } else {
                inside.to_string()
            };
            if !dist.join(&file).is_file() {
                c.broken.push((rel.clone(), href.to_string()));
                continue;
            }
            let is_html = Path::new(&file)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("html"));
            if is_html {
                if seen.insert(file.clone()) {
                    queue.push_back(file);
                }
            } else {
                images.insert(file);
            }
        }
    }
    // Every built page, linked or not (the 404 page too), opens light by default.
    let mut stack = vec![dist.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| Error::io("read dir", &dir, e))?;
        for entry in entries {
            let path = entry.map_err(|e| Error::io("read dir", &dir, e))?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("html"))
            {
                let html =
                    std::fs::read_to_string(&path).map_err(|e| Error::io("read", &path, e))?;
                c.themed += 1;
                if let Some(reason) = theme_problem(&html) {
                    let rel = path
                        .strip_prefix(dist)
                        .unwrap_or(&path)
                        .display()
                        .to_string();
                    c.theme_failures.push((rel, reason.to_string()));
                }
            }
        }
    }
    let rel_link = |page: &str, rel: &str| -> Option<String> {
        let html = std::fs::read_to_string(dist.join(page)).ok()?;
        let key = format!("rel=\"{rel}\" href=\"");
        let at = html.find(&key)? + key.len();
        let rest = html.get(at..)?;
        Some(rest.get(..rest.find('"')?)?.to_string())
    };
    c.loop_ok = rel_link("revelation/22/index.html", "next")
        == Some(format!("{base_path}genesis/1/"))
        && rel_link("genesis/1/index.html", "prev") == Some(format!("{base_path}revelation/22/"));
    c.chapters = seen.iter().filter(|r| is_chapter(r)).count();
    c.books = seen.iter().filter(|r| is_book(r)).count();
    c.images = images.len();
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_and_book_paths_are_recognised() {
        assert!(is_chapter("john/3/index.html"));
        assert!(is_book("song-of-solomon/index.html"));
        assert!(!is_chapter("art/the-creation-of-light/index.html"));
        assert!(!is_book("lectionary/index.html"));
    }

    #[test]
    fn srcset_candidates_are_each_checked() {
        let html = "<source srcset=\"/a.avif 480w, /b.avif 720w\"><img srcset=\"/c.jpg 1x\">";
        assert_eq!(srcset_urls(html), vec!["/a.avif", "/b.avif", "/c.jpg"]);
    }

    #[test]
    fn a_page_that_would_open_dark_is_caught() {
        let good = "<!doctype html><html lang=\"en\"><head><meta name=\"color-scheme\" content=\"light\"><style>:root { --paper: #ffffff; } :root[data-theme=\"dark\"] { --paper: #0e0e0e; }</style></head><body></body></html>";
        assert_eq!(theme_problem(good), None);
        let dark = good.replace(
            "<html lang=\"en\">",
            "<html lang=\"en\" data-theme=\"dark\">",
        );
        assert!(theme_problem(&dark).is_some());
        let system = good.replace("</style>", "@media (prefers-color-scheme: dark) {}</style>");
        assert!(theme_problem(&system).is_some());
        let bare = good.replace("--paper: #ffffff", "--x: 0");
        assert!(theme_problem(&bare).is_some());
    }

    #[test]
    fn attribute_values_are_read_in_order() {
        let html = "<a href=\"/a/\">x</a><img src=\"/i.jpg\"><a href=\"/b/#v1\">";
        assert_eq!(attr_values(html, "href"), vec!["/a/", "/b/#v1"]);
        assert_eq!(attr_values(html, "src"), vec!["/i.jpg"]);
    }
}

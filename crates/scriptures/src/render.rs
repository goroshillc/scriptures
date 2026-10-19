//! Page renderers: home, book, chapter, art, lectionary, about and 404.
//!
//! > *"And he said unto them, Go ye into all the world, and preach the gospel to every
//! > creature."* - Mark 16:15, KJV
//!
//! WHY everything is pre-rendered: a chapter must be readable, linkable and indexable
//! the moment its HTML arrives - no client fetch, no hydration. Cross-references sit in
//! `<details>` under each verse (the reader's "tap a verse" interaction, with no
//! script); verse anchors are `#v<N>`; the words of Christ carry `class="wj"`.

use crate::art::{Artwork, Manifest, Variant};
use crate::bible::Bible;
use crate::canon::{self, Testament};
use crate::commons::artist_facts;
use crate::features;
use crate::html::{self, OgImage, PageMeta, esc};
use crate::lectionary::Lectionary;
use crate::redletter::RedLetters;
use crate::site::SiteConfig;
use crate::xref::{self, Xrefs};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Verses of a cross-referenced range shown inline before "…" (the link opens it all).
pub const XREF_SHOWN_VERSES: usize = 6;

/// `sizes` for a wide plate (up to 60rem, beyond the 44rem text column).
pub const PLATE_SIZES: &str = "(max-width: 62.5rem) calc(100vw - 2.5rem), 60rem";
/// `sizes` for a gallery tile.
pub const TILE_SIZES: &str = "(max-width: 30rem) calc(100vw - 2.5rem), 15rem";

/// Final image files for one work: JPEG (metadata-stamped), WebP and AVIF at each width.
#[derive(Debug, Clone, Default)]
pub struct Served {
    /// JPEG display variants (width, height, site-relative path), narrowest first.
    pub variants: Vec<(u32, u32, String)>,
    /// WebP at the same widths, when the crisp derivatives exist.
    pub webp: Vec<(u32, u32, String)>,
    /// AVIF at the same widths, when the crisp derivatives exist.
    pub avif: Vec<(u32, u32, String)>,
    /// The full scan for the zoom view: (width, height, JPEG, WebP, AVIF).
    pub full: Option<(u32, u32, String, String, String)>,
    /// Social card (width, height, site-relative path).
    pub og: Option<(u32, u32, String)>,
}

/// Everything a renderer reads.
pub struct Ctx<'a> {
    /// URLs and dates.
    pub cfg: &'a SiteConfig,
    /// The text.
    pub bible: &'a Bible,
    /// The apparatus.
    pub xrefs: &'a Xrefs,
    /// The words of Christ.
    pub red: &'a RedLetters,
    /// The art.
    pub art: &'a Manifest,
    /// Served image files by work slug.
    pub served: &'a HashMap<String, Served>,
    /// The lectionary window, when fetched.
    pub lect: Option<&'a Lectionary>,
    /// Licence evidence for the site icon, when one ships.
    pub icon: Option<&'a crate::commons::IconRecord>,
    /// Psalm 115:1 with the site's motto and the verified original languages.
    pub psalm: Option<&'a crate::features::Psalm115>,
    /// The computus compiled to WebAssembly, for the any-year Paschalion.
    pub wasm: Option<&'a [u8]>,
}

impl Ctx<'_> {
    fn book_href(&self, book: usize) -> String {
        self.cfg
            .href(&format!("{}/", canon::book(book).map_or("", |b| b.slug)))
    }

    fn chapter_path(book: usize, chapter: usize) -> String {
        format!("{}/{chapter}/", canon::book(book).map_or("", |b| b.slug))
    }

    /// Link to a verse anchor (shared with the feature components).
    pub(crate) fn verse_href_pub(&self, book: usize, chapter: usize, verse: usize) -> String {
        self.verse_href(book, chapter, verse)
    }

    /// A figure sized for a two-column feature block.
    pub(crate) fn feature_figure(&self, w: &Artwork) -> String {
        self.figure(
            w,
            false,
            "(min-width: 52rem) 30rem, calc(100vw - 5rem)",
            true,
            false,
        )
    }

    fn verse_href(&self, book: usize, chapter: usize, verse: usize) -> String {
        format!(
            "{}#v{verse}",
            self.cfg.href(&Self::chapter_path(book, chapter))
        )
    }

    fn name(&self, book: usize) -> &str {
        self.bible.book_name(book).unwrap_or("")
    }

    /// One verse as HTML with the words of Christ marked.
    #[must_use]
    pub fn verse_html(&self, book: usize, chapter: usize, verse: usize) -> String {
        let Some(text) = self.bible.verse_text(book, chapter, verse) else {
            return String::new();
        };
        red_html(&text, self.red.spans(&xref::osis_key(book, chapter, verse)))
    }

    fn src_set(&self, s: &Served) -> String {
        self.set_of(&s.variants)
    }

    fn set_of(&self, list: &[(u32, u32, String)]) -> String {
        list.iter()
            .map(|(w, _, p)| format!("{} {w}w", self.cfg.href(p)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// `<picture>`: AVIF, then WebP, then the JPEG every browser reads, with the true
    /// width and height so nothing shifts while it loads.
    fn picture(&self, w: &Artwork, s: &Served, sizes: &str, eager: bool) -> String {
        let Some((width, height, src)) = s.variants.last() else {
            return String::new();
        };
        let loading = if eager {
            "fetchpriority=\"high\" decoding=\"async\""
        } else {
            "loading=\"lazy\" decoding=\"async\""
        };
        let mut h = String::from("<picture>");
        for (mime, list) in [("image/avif", &s.avif), ("image/webp", &s.webp)] {
            if !list.is_empty() {
                let _ = write!(
                    h,
                    "<source type=\"{mime}\" srcset=\"{}\" sizes=\"{}\">",
                    esc(&self.set_of(list)),
                    esc(sizes)
                );
            }
        }
        let _ = write!(
            h,
            "<img src=\"{}\" srcset=\"{}\" sizes=\"{}\" width=\"{width}\" height=\"{height}\" alt=\"{}\" {loading}></picture>",
            esc(&self.cfg.href(src)),
            esc(&self.src_set(s)),
            esc(sizes),
            esc(&alt_text(w, self))
        );
        h
    }

    pub(crate) fn og_pub(&self, w: &Artwork) -> Option<OgImage> {
        self.og_for(w)
    }

    fn og_for(&self, w: &Artwork) -> Option<OgImage> {
        let s = self.served.get(&w.slug)?;
        let (ow, oh, p) = s.og.as_ref()?;
        Some(OgImage {
            url: self.cfg.abs(p),
            width: *ow,
            height: *oh,
            alt: alt_text(w, self),
        })
    }

    fn passage_label(&self, w: &Artwork) -> String {
        w.passage.as_deref().map_or_else(
            || w.other_ref.clone().unwrap_or_default(),
            |p| pretty_range(self.bible, p),
        )
    }

    fn passage_href(&self, w: &Artwork) -> Option<String> {
        let s = w.start()?;
        Some(self.verse_href(s.book, s.chapter, s.verse))
    }

    /// A `<figure>` for a work; `eager` marks the likely LCP image.
    /// A wide plate in the reading flow: given room beyond the text column.
    fn plate(&self, w: &Artwork, eager: bool, link_page: bool) -> String {
        self.figure(w, eager, PLATE_SIZES, link_page, true)
    }

    fn figure(&self, w: &Artwork, eager: bool, sizes: &str, link_page: bool, wide: bool) -> String {
        let Some(s) = self.served.get(&w.slug) else {
            return String::new();
        };
        let Some((width, _, _)) = s.variants.last() else {
            return String::new();
        };
        let picture = self.picture(w, s, sizes, eager);
        let page = self.cfg.href(&format!("art/{}/", w.slug));
        // On a reading page the plate opens its own page; on its own page it opens the
        // full scan (a plain link that works without scripting; the zoom view with it).
        let pic = if link_page {
            format!("<a class=\"mat\" href=\"{}\">{picture}</a>", esc(&page))
        } else if let Some((fw, fh, jpg, webp, avif)) = &s.full {
            // The desktop edition ships the zoom image as JPEG only: no empty sources.
            let mut formats = String::new();
            for (name, file) in [("avif", avif), ("webp", webp)] {
                if !file.is_empty() {
                    let _ = write!(formats, " data-{name}=\"{}\"", esc(&self.cfg.href(file)));
                }
            }
            format!(
                "<a class=\"mat zoom\" href=\"{}\"{formats} data-width=\"{fw}\" data-height=\"{fh}\" aria-label=\"Open the full scan, {fw} by {fh} pixels\">{picture}</a>",
                esc(&self.cfg.href(jpg)),
            )
        } else {
            format!("<span class=\"mat\">{picture}</span>")
        };
        let passage = self.passage_href(w).map_or_else(
            || esc(&self.passage_label(w)),
            |h| {
                format!(
                    "<a href=\"{}\">{}</a>",
                    esc(&h),
                    esc(&self.passage_label(w))
                )
            },
        );
        let facts = artist_facts(&w.artist_key);
        let artist = facts.map_or(w.artist.as_str(), |f| f.name);
        // A master narrower than the column is shown at its own size, never upscaled.
        let class = if *width < 640 {
            " class=\"small\""
        } else if wide {
            " class=\"plate\""
        } else {
            ""
        };
        let data = format!(" data-plate=\"{}\"", esc(&w.slug));
        format!(
            "<figure{class}{data}>{pic}<figcaption><span class=\"t\">{}</span> \u{b7} {} \u{b7} {}{} \u{b7} {passage}<br>Public domain \u{b7} <a href=\"{}\">Wikimedia Commons</a> \u{b7} <a href=\"{}\">About this work</a></figcaption></figure>",
            esc(&w.scene),
            esc(&w.medium),
            esc(artist),
            if w.date.is_empty() {
                String::new()
            } else {
                format!(", {}", esc(&w.date))
            },
            esc(&w.commons_url),
            esc(&page)
        )
    }

    /// A gallery tile: the plate on its white field, then its passage and its title.
    pub(crate) fn tile_pub(&self, w: &Artwork) -> String {
        let Some(s) = self.served.get(&w.slug) else {
            return String::new();
        };
        let page = self.cfg.href(&format!("art/{}/", w.slug));
        let book = w.start().map_or_else(
            || "beyond-66".to_string(),
            |l| canon::book(l.book).map_or("", |b| b.slug).to_string(),
        );
        format!(
            "<li data-book=\"{}\"><figure><a class=\"mat\" href=\"{}\">{}</a><figcaption><span class=\"ref\">{}</span><a href=\"{}\"><span class=\"t\">{}</span></a></figcaption></figure></li>",
            esc(&book),
            esc(&page),
            self.picture(w, s, TILE_SIZES, false),
            esc(&self.passage_label(w)),
            esc(&page),
            esc(&w.scene)
        )
    }
}

/// Alt text: what the image is and what it illustrates.
fn alt_text(w: &Artwork, ctx: &Ctx<'_>) -> String {
    let artist =
        artist_facts(&w.artist_key).map_or_else(|| w.artist.clone(), |f| f.name.to_string());
    let date = if w.date.is_empty() {
        String::new()
    } else {
        format!(" ({})", w.date)
    };
    format!(
        "{}: {} by {artist}{date}, illustrating {}",
        w.scene,
        w.medium.to_lowercase(),
        ctx.passage_label(w)
    )
}

/// Escapes a verse and wraps the given byte spans in `<span class="wj">`.
#[must_use]
pub fn red_html(text: &str, spans: &[(usize, usize)]) -> String {
    let mut out = String::with_capacity(text.len() + spans.len() * 26);
    let mut at = 0usize;
    for &(s, e) in spans {
        if s < at || e > text.len() || s >= e {
            continue;
        }
        let (Some(before), Some(red)) = (text.get(at..s), text.get(s..e)) else {
            continue;
        };
        out.push_str(&esc(before));
        let _ = write!(out, "<span class=\"wj\">{}</span>", esc(red));
        at = e;
    }
    out.push_str(&esc(text.get(at..).unwrap_or("")));
    out
}

/// As [`red_html`], and the words around His (the narrative) wrapped in
/// `<span class="n">`, so the "words of Christ only" reading can quiet them.
#[must_use]
pub fn red_narrative_html(text: &str, spans: &[(usize, usize)]) -> String {
    if spans.is_empty() {
        return esc(text);
    }
    let mut out = String::with_capacity(text.len() + spans.len() * 48);
    let mut at = 0usize;
    let narrative = |out: &mut String, part: &str| {
        if !part.is_empty() {
            let _ = write!(out, "<span class=\"n\">{}</span>", esc(part));
        }
    };
    for &(s, e) in spans {
        if s < at || e > text.len() || s >= e {
            continue;
        }
        let (Some(before), Some(red)) = (text.get(at..s), text.get(s..e)) else {
            continue;
        };
        narrative(&mut out, before);
        let _ = write!(out, "<span class=\"wj\">{}</span>", esc(red));
        at = e;
    }
    narrative(&mut out, text.get(at..).unwrap_or(""));
    out
}

/// "Genesis 1:1–5", "Exodus 1:8–2:4", "Mark 14:45" from an OSIS passage.
#[must_use]
pub fn pretty_range(bible: &Bible, osis: &str) -> String {
    xref::human(bible, osis).replace('-', "\u{2013}")
}

fn truncate_words(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for word in s.split_whitespace() {
        if out.chars().count() + word.chars().count() + 1 > max.saturating_sub(1) {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    let trimmed = out.trim_end_matches([',', ';', ':', '.']).to_string();
    format!("{trimmed}\u{2026}")
}

fn crumbs(ctx: &Ctx<'_>, items: &[(String, Option<String>)]) -> String {
    let mut h = String::from("<nav class=\"crumbs\" aria-label=\"Breadcrumb\"><ol>");
    for (label, href) in items {
        match href {
            Some(u) => {
                let _ = write!(h, "<li><a href=\"{}\">{}</a></li>", esc(u), esc(label));
            }
            None => {
                let _ = write!(h, "<li aria-current=\"page\">{}</li>", esc(label));
            }
        }
    }
    let _ = ctx;
    h.push_str("</ol></nav>");
    h
}

fn breadcrumb_ld(ctx: &Ctx<'_>, items: &[(String, String)]) -> Value {
    let list: Vec<Value> = items
        .iter()
        .enumerate()
        .map(|(i, (name, path))| json!({"@type": "ListItem", "position": i + 1, "name": name, "item": ctx.cfg.abs(path)}))
        .collect();
    json!({"@type": "BreadcrumbList", "itemListElement": list})
}

fn publisher() -> Value {
    json!({"@type": "Organization", "name": "Goroshi LLC", "url": "https://goroshillc.com/"})
}

fn bible_ld(ctx: &Ctx<'_>) -> Value {
    json!({
        "@type": "Book",
        "@id": ctx.cfg.abs("#bible"),
        "name": "The Holy Bible, King James Version",
        "bookEdition": "King James Version (1769 standard text)",
        "inLanguage": "en",
        "url": ctx.cfg.abs(""),
        "publisher": publisher()
    })
}

/// The home page.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn home(ctx: &Ctx<'_>, featured: Option<&Artwork>) -> String {
    let mut b = String::from("<div class=\"wrap wide\"><section class=\"hero\">");
    let _ = write!(
        b,
        concat!(
            "{emblem}<p class=\"eyebrow\">The Holy Bible · King James Version</p><h1>The Scriptures</h1>",
            "<p class=\"gk\" lang=\"grc\">Ἡ Ἁγία Γραφή</p>",
            "<ul class=\"counts\"><li><strong>66</strong>books</li><li><strong>{ch}</strong>chapters</li><li><strong>{vs}</strong>verses</li><li><strong>{xr}</strong>cross-references</li><li><strong>{art}</strong>master works</li></ul>",
            "<p class=\"continue\" id=\"continue\" hidden></p></section>"
        ),
        emblem = html::emblem("halo-lg"),
        ch = thousands(ctx.bible.total_chapters()),
        vs = thousands(ctx.bible.total_verses()),
        xr = thousands(ctx.xrefs.total_links()),
        art = ctx.art.works.len(),
    );
    let _ = write!(
        b,
        "<section class=\"panel\" aria-label=\"Discover\"><h2>Where would you like to begin?</h2><p>Find a remembered verse, explore a subject, or follow a work of art into its passage.</p><div class=\"topic-chips\"><a class=\"discover-button\" href=\"{}\">Search Scripture</a><a class=\"topic-chip\" href=\"{}\">Explore artwork</a><a class=\"topic-chip\" href=\"{}\">Find an Orthodox parish</a></div></section>",
        esc(&ctx.cfg.href("search/")),
        esc(&ctx.cfg.href("art/")),
        esc(&ctx.cfg.href("parishes/"))
    );
    let _ = write!(
        b,
        "<div class=\"narrow\"><blockquote><p>{}</p><cite><a href=\"{}\">John 1:1</a></cite></blockquote></div>",
        ctx.verse_html(42, 1, 1),
        esc(&ctx.verse_href(42, 1, 1))
    );
    b.push_str("<section aria-labelledby=\"books\"><h2 id=\"books\" class=\"center\">Choose a book</h2><p class=\"sub center\">Every chapter of the King James Bible is one click from here, with each verse's cross-references resolved to their text and the words of Christ in red.</p>");
    b.push_str(&features::book_selector(ctx, None, 3));
    b.push_str("</section>");
    if let Some(w) = featured {
        let _ = write!(
            b,
            "<section class=\"feature flip\" aria-labelledby=\"luke137\">{}<div><p class=\"eyebrow\">Luke 1</p><h2 id=\"luke137\">The Annunciation</h2><blockquote><p>{}</p><cite><a href=\"{}\">Luke 1:37</a></cite></blockquote></div></section>",
            ctx.feature_figure(w),
            ctx.verse_html(41, 1, 37),
            esc(&ctx.verse_href(41, 1, 37))
        );
    }
    let dry_bones = ctx
        .art
        .works
        .iter()
        .find(|w| w.start().is_some_and(|s| s.book == 25 && s.chapter == 37));
    b.push_str(&features::ezekiel_37(ctx, dry_bones));
    if let Some(day) = ctx
        .lect
        .and_then(|l| l.days.iter().find(|d| d.date == ctx.cfg.build_date))
    {
        let _ = write!(
            b,
            "<section class=\"panel\" aria-labelledby=\"lh\"><p class=\"eyebrow\">The Orthodox lectionary</p><h2 id=\"lh\" class=\"flush\">Readings for {}</h2>",
            esc(&day.date)
        );
        b.push_str(&day_html(ctx, day));
        let _ = write!(
            b,
            "<p><a href=\"{}\">The lectionary, and the Paschalion for any year</a></p></section>",
            esc(&ctx.cfg.href("lectionary/"))
        );
    }
    if let Some(p) = ctx.psalm {
        b.push_str(&features::non_nobis(ctx, p, false));
    }
    let _ = write!(
        b,
        "<section class=\"center\"><h2>The art</h2><p>{} public-domain works by Gustave Doré, Albrecht Dürer and Titian, each placed beside the passage it illustrates and credited with its Wikimedia Commons licence.</p><p><a href=\"{}\">See all the art, book by book</a></p></section>",
        ctx.art.works.len(),
        esc(&ctx.cfg.href("art/"))
    );
    b.push_str("</div>");
    let desc = format!(
        "The King James Bible with {} cross-references resolved to their text, the words of Christ in red, the Orthodox lectionary and Doré's engravings.",
        thousands(ctx.xrefs.total_links())
    );
    let website = json!({
        "@type": "WebSite",
        "@id": ctx.cfg.abs("#site"),
        "name": "The Scriptures",
        "alternateName": "Ἡ Ἁγία Γραφή",
        "url": ctx.cfg.abs(""),
        "inLanguage": "en",
        "description": desc,
        "publisher": publisher(),
        "copyrightHolder": [{"@type": "Person", "name": "Stefano Theofanous"}, publisher()]
    });
    let meta = PageMeta {
        title: "The Scriptures · The Holy Bible, King James Version".to_string(),
        description: desc,
        path: String::new(),
        og: featured.and_then(|w| ctx.og_for(w)),
        og_type: "website",
        json_ld: vec![website, bible_ld(ctx)],
        section: "books",
        noindex: false,
        preload_image: None,
        extra_script: None,
        own_search: false,
        module: None,
    };
    html::page(ctx.cfg, &meta, &b)
}

/// A book page: chapters and the book's art.
#[must_use]
pub fn book(ctx: &Ctx<'_>, bi: usize) -> String {
    let name = ctx.name(bi).to_string();
    let chapters = ctx.bible.chapter_count(bi);
    let verses: usize = (1..=chapters).map(|c| ctx.bible.verse_count(bi, c)).sum();
    let testament = canon::book(bi).map_or(Testament::Old, |b| b.testament);
    let t_label = if testament == Testament::Old {
        "Old Testament"
    } else {
        "New Testament"
    };
    let mut b = String::from("<div class=\"wrap\">");
    b.push_str(&crumbs(
        ctx,
        &[
            ("The Scriptures".into(), Some(ctx.cfg.href(""))),
            (name.clone(), None),
        ],
    ));
    let _ = write!(
        b,
        "<h1>{}</h1><p class=\"sub\">{t_label} · {chapters} chapter{} · {} verses · King James Version</p>",
        esc(&name),
        if chapters == 1 { "" } else { "s" },
        thousands(verses)
    );
    b.push_str("<h2 class=\"sr\">Chapters</h2><ul class=\"grid\">");
    for c in 1..=chapters {
        let _ = write!(
            b,
            "<li><a href=\"{}\" aria-label=\"{} chapter {c}\">{c}</a></li>",
            esc(&ctx.cfg.href(&Ctx::chapter_path(bi, c))),
            esc(&name)
        );
    }
    b.push_str("</ul>");
    let works = ctx.art.in_book(bi);
    if !works.is_empty() {
        let _ = write!(b, "<h2>Art for {}</h2><ul class=\"gallery\">", esc(&name));
        for w in &works {
            b.push_str(&ctx.tile_pub(w));
        }
        b.push_str("</ul>");
    }
    let _ = write!(
        b,
        "<section aria-labelledby=\"allbooks\"><h2 id=\"allbooks\">All 66 books</h2>{}</section>",
        features::book_selector(ctx, Some(bi), 3)
    );
    let last = canon::BOOKS.len() - 1;
    let before = if bi == 0 { last } else { bi - 1 };
    let after = if bi >= last { 0 } else { bi + 1 };
    b.push_str(&pager(
        Some((ctx.book_href(before), ctx.name(before).to_string())),
        Some((ctx.cfg.href(""), "All books".to_string())),
        Some((ctx.book_href(after), ctx.name(after).to_string())),
    ));
    b.push_str("</div>");
    let first = ctx.bible.verse_text(bi, 1, 1).unwrap_or_default();
    let desc = truncate_words(
        &format!(
            "{name} in the King James Version: {chapters} chapters with every cross-reference resolved to its text. {first}"
        ),
        158,
    );
    let path = format!("{}/", canon::book(bi).map_or("", |x| x.slug));
    let book_ld = json!({
        "@type": "Book",
        "name": name,
        "url": ctx.cfg.abs(&path),
        "inLanguage": "en",
        "isPartOf": {"@id": ctx.cfg.abs("#bible")},
        "numberOfPages": chapters
    });
    let meta = PageMeta {
        title: format!("{name} (KJV) · The Scriptures"),
        description: desc,
        path: path.clone(),
        og: works.first().and_then(|w| ctx.og_for(w)),
        og_type: "book",
        json_ld: vec![
            breadcrumb_ld(
                ctx,
                &[("The Scriptures".into(), String::new()), (name, path)],
            ),
            book_ld,
            bible_ld(ctx),
        ],
        section: "books",
        noindex: false,
        preload_image: None,
        extra_script: None,
        own_search: false,
        module: None,
    };
    html::page(ctx.cfg, &meta, &b)
}

fn pager(
    prev: Option<(String, String)>,
    mid: Option<(String, String)>,
    next: Option<(String, String)>,
) -> String {
    let mut h = String::from("<nav class=\"pager\" aria-label=\"Pages\">");
    match prev {
        Some((u, l)) => {
            let _ = write!(
                h,
                "<a rel=\"prev\" href=\"{}\">\u{2190} {}</a>",
                esc(&u),
                esc(&l)
            );
        }
        None => h.push_str("<span></span>"),
    }
    if let Some((u, l)) = mid {
        let _ = write!(h, "<a href=\"{}\">{}</a>", esc(&u), esc(&l));
    }
    match next {
        Some((u, l)) => {
            let _ = write!(
                h,
                "<a rel=\"next\" href=\"{}\">{} \u{2192}</a>",
                esc(&u),
                esc(&l)
            );
        }
        None => h.push_str("<span></span>"),
    }
    h.push_str("</nav>");
    h
}

/// The resolved text of a cross-reference, red letters kept, long ranges shortened.
fn xref_text_html(ctx: &Ctx<'_>, osis: &str) -> String {
    let locs = xref::expand_range(ctx.bible, osis);
    let mut parts = Vec::new();
    let mut shown = 0usize;
    let mut more = false;
    for p in &locs {
        if ctx.bible.verse_text(p.book, p.chapter, p.verse).is_none() {
            continue;
        }
        if shown == XREF_SHOWN_VERSES {
            more = true;
            break;
        }
        parts.push(ctx.verse_html(p.book, p.chapter, p.verse));
        shown += 1;
    }
    let mut s = parts.join(" ");
    if more {
        s.push_str(" <span class=\"more\">\u{2026}</span>");
    }
    s
}

/// The verses of a chapter with their apparatus and art, the reference count, and the
/// work that will be the largest early paint (if any).
fn chapter_text<'w>(
    ctx: &Ctx<'_>,
    bi: usize,
    c: usize,
    works: &[&'w Artwork],
) -> (String, usize, Option<&'w Artwork>) {
    let n_verses = ctx.bible.verse_count(bi, c);
    let mut placed = 0usize;
    let mut total_refs = 0usize;
    let mut body = String::new();
    let mut lcp: Option<&Artwork> = None;
    for v in 1..=n_verses {
        while let Some(w) = works.get(placed) {
            if w.start().is_some_and(|s| s.verse <= v) {
                let eager = placed == 0 && v <= 3;
                if eager {
                    lcp = Some(w);
                }
                body.push_str(&ctx.plate(w, eager, true));
                placed += 1;
            } else {
                break;
            }
        }
        let key = xref::osis_key(bi, c, v);
        let Some(full) = xref::get_verse(ctx.bible, ctx.xrefs, &key) else {
            continue;
        };
        let text = red_narrative_html(&full.text, ctx.red.spans(&key));
        // Every verse opens: its permalink (and, with scripting, copy and keep for
        // memory), then its cross-references resolved to their text.
        let n = full.cross_refs.len();
        total_refs += n;
        let chip = if n == 0 {
            String::new()
        } else {
            format!(
                "<span class=\"xc\">{n}<span class=\"sr\"> cross-reference{}</span></span>",
                if n == 1 { "" } else { "s" }
            )
        };
        let _ = write!(
            body,
            "<div class=\"v\" id=\"v{v}\"><details><summary><span class=\"vn\">{v}</span>{text}{chip}</summary><div class=\"xr\"><p class=\"vt\"><a href=\"#v{v}\" data-ref=\"{} {c}:{v}\">Link to this verse</a></p>",
            esc(ctx.name(bi))
        );
        if n > 0 {
            body.push_str("<ol>");
            for x in &full.cross_refs {
                let href = xref::parse_one(x.osis.split('-').next().unwrap_or(""))
                    .map_or_else(String::new, |p| ctx.verse_href(p.book, p.chapter, p.verse));
                let _ = write!(
                    body,
                    "<li><a href=\"{}\" data-xref>{}</a>{}</li>",
                    esc(&href),
                    esc(&x.reference.replace('-', "\u{2013}")),
                    xref_text_html(ctx, &x.osis)
                );
            }
            body.push_str("</ol>");
        }
        body.push_str("</div></details></div>");
    }
    for w in works.iter().skip(placed) {
        body.push_str(&ctx.plate(w, false, true));
    }
    (body, total_refs, lcp)
}

type Link = Option<(String, String)>;

/// The chapter before and after, crossing book boundaries and looping: the reading
/// passes from Revelation 22 to Genesis 1, and back from Genesis 1 to Revelation 22.
#[must_use]
pub fn chapter_loop(bible: &Bible, bi: usize, c: usize) -> ((usize, usize), (usize, usize)) {
    let last_book = canon::BOOKS.len() - 1;
    let prev = if c > 1 {
        (bi, c - 1)
    } else {
        let p = if bi == 0 { last_book } else { bi - 1 };
        (p, bible.chapter_count(p))
    };
    let next = if c < bible.chapter_count(bi) {
        (bi, c + 1)
    } else {
        (if bi >= last_book { 0 } else { bi + 1 }, 1)
    };
    (prev, next)
}

/// Previous and next chapter links (always present: the reading is a loop).
fn chapter_neighbours(ctx: &Ctx<'_>, bi: usize, c: usize) -> (Link, Link) {
    let ((pb, pc), (nb, nc)) = chapter_loop(ctx.bible, bi, c);
    let link = |b: usize, ch: usize| {
        Some((
            ctx.cfg.href(&Ctx::chapter_path(b, ch)),
            format!("{} {ch}", ctx.name(b)),
        ))
    };
    (link(pb, pc), link(nb, nc))
}

/// A chapter page: every verse, its apparatus, and the art placed at its passage.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn chapter(ctx: &Ctx<'_>, bi: usize, c: usize) -> String {
    let name = ctx.name(bi).to_string();
    let n_verses = ctx.bible.verse_count(bi, c);
    let works = ctx.art.in_chapter(bi, c);
    let (body, total_refs, lcp) = chapter_text(ctx, bi, c, &works);
    let path = Ctx::chapter_path(bi, c);
    let mut b = String::from("<div class=\"wrap\">");
    b.push_str(&crumbs(
        ctx,
        &[
            ("The Scriptures".into(), Some(ctx.cfg.href(""))),
            (name.clone(), Some(ctx.book_href(bi))),
            (format!("Chapter {c}"), None),
        ],
    ));
    b.push_str(&features::picker(ctx, Some(bi), false));
    let word_count: usize = (1..=n_verses)
        .filter_map(|v| ctx.bible.verse_text(bi, c, v))
        .map(|t| t.split_whitespace().count())
        .sum();
    let has_red = (1..=n_verses).any(|v| !ctx.red.spans(&xref::osis_key(bi, c, v)).is_empty());
    let slug = canon::book(bi).map_or("", |x| x.slug);
    let _ = write!(
        b,
        "<article aria-labelledby=\"h\" data-book=\"{slug}\" data-chapter=\"{c}\" data-verses=\"{n_verses}\" data-words=\"{word_count}\"><h1 id=\"h\">{} {c}</h1><p class=\"sub\">King James Version \u{b7} {n_verses} verses \u{b7} {} cross-references \u{b7} open a verse to read them</p>",
        esc(&name),
        thousands(total_refs)
    );
    if has_red {
        b.push_str("<div class=\"chapter-tools\"><label class=\"toggle\"><input type=\"checkbox\" id=\"wj-only\"> Words of Christ only</label></div>");
    }
    let _ = write!(
        b,
        "<div class=\"text\">{body}</div><div id=\"chapter-end\" aria-hidden=\"true\"></div>"
    );
    if bi == canon::BOOKS.len() - 1 && c == ctx.bible.chapter_count(bi) {
        // After Revelation 22:21, only a quiet line onward: the reading begins again.
        let _ = write!(
            b,
            "<p class=\"onward\"><a href=\"{}\">{}</a><span>Genesis 1:1</span></p>",
            esc(&ctx.verse_href(0, 1, 1)),
            esc(&ctx.bible.verse_text(0, 1, 1).unwrap_or_default())
        );
    }
    b.push_str("</article>");
    let (prev, next) = chapter_neighbours(ctx, bi, c);
    b.push_str(&pager(
        prev,
        Some((ctx.book_href(bi), format!("All of {name}"))),
        next,
    ));
    b.push_str("</div>");
    let first = ctx.bible.verse_text(bi, c, 1).unwrap_or_default();
    let desc = truncate_words(
        &format!(
            "{name} {c} KJV: {first} Read with {} cross-references and the words of Christ in red.",
            thousands(total_refs)
        ),
        160,
    );
    let og_work = works
        .first()
        .copied()
        .or_else(|| ctx.art.in_book(bi).first().copied());
    let mut chapter_ld = json!({
        "@type": "Chapter",
        "name": format!("{name} {c}"),
        "position": c,
        "url": ctx.cfg.abs(&path),
        "inLanguage": "en",
        "isPartOf": {"@type": "Book", "name": name, "url": ctx.cfg.abs(&format!("{}/", canon::book(bi).map_or("", |x| x.slug))), "isPartOf": {"@id": ctx.cfg.abs("#bible")}},
        "publisher": publisher()
    });
    if let (Some(og), Some(obj)) = (
        og_work.and_then(|w| ctx.og_for(w)),
        chapter_ld.as_object_mut(),
    ) {
        obj.insert("image".into(), json!(og.url));
    }
    let preload = lcp.and_then(|w| {
        let s = ctx.served.get(&w.slug)?;
        let (_, _, p) = s.variants.last()?;
        Some((ctx.cfg.href(p), ctx.src_set(s)))
    });
    let meta = PageMeta {
        title: format!("{name} {c} (KJV) · The Scriptures"),
        description: desc,
        path: path.clone(),
        og: og_work.and_then(|w| ctx.og_for(w)),
        og_type: "article",
        json_ld: vec![
            breadcrumb_ld(
                ctx,
                &[
                    ("The Scriptures".into(), String::new()),
                    (
                        name.clone(),
                        format!("{}/", canon::book(bi).map_or("", |x| x.slug)),
                    ),
                    (format!("{name} {c}"), path),
                ],
            ),
            chapter_ld,
        ],
        section: "books",
        noindex: false,
        preload_image: preload,
        extra_script: None,
        own_search: false,
        // Loads the Rust highlighter only when the page was opened from a search (`?hl=`).
        module: Some(format!("discovery/ui.js?v={}", ctx.cfg.discovery_version)),
    };
    html::page(ctx.cfg, &meta, &b)
}

/// One lectionary day as a list of facts and linked readings.
fn day_html(ctx: &Ctx<'_>, day: &crate::lectionary::Day) -> String {
    let mut h = String::new();
    if !day.titles.is_empty() {
        let _ = write!(h, "<p>{}</p>", esc(&day.titles.join("; ")));
    }
    if !day.saints.is_empty() {
        let _ = write!(
            h,
            "<p class=\"meta\">Commemorated: {}</p>",
            esc(&day.saints.join("; "))
        );
    }
    let mut facts = Vec::new();
    if !day.fast.is_empty() {
        facts.push(esc(&day.fast));
    }
    if let Some(t) = day.tone {
        facts.push(format!("Tone {t}"));
    }
    if !facts.is_empty() {
        let _ = write!(h, "<p class=\"meta\">{}</p>", facts.join(" · "));
    }
    if !day.readings.is_empty() {
        h.push_str("<ul>");
        for r in &day.readings {
            let label = if r.source.is_empty() {
                String::new()
            } else {
                format!("{}: ", esc(&r.source))
            };
            match (r.book, r.start) {
                (Some(b), Some((c, v))) => {
                    let _ = write!(
                        h,
                        "<li>{label}<a href=\"{}\">{}</a></li>",
                        esc(&ctx.verse_href(b, c, v)),
                        esc(&r.display)
                    );
                }
                _ => {
                    let _ = write!(h, "<li>{label}{}</li>", esc(&r.display));
                }
            }
        }
        h.push_str("</ul>");
    }
    h
}

/// The Paschalion: the movable cycle for any year, computed (never looked up).
fn paschalion_section(ctx: &Ctx<'_>) -> String {
    let year: i64 = ctx.cfg.year().parse().unwrap_or(2026);
    let mut h = String::from(
        "<section aria-labelledby=\"pasch\"><p class=\"eyebrow\">The Paschalion</p><h2 id=\"pasch\" class=\"flush\">Pascha and the movable feasts, any year</h2>",
    );
    h.push_str("<p>Pascha is computed by the Julian computus the Orthodox Church keeps, and every movable feast is counted from it, so any year opens at once, the year 40,000 as exactly as this one. Dates are given on the civil (Gregorian) calendar, with Pascha also on the Julian calendar; before 1582 the civil date is the proleptic Gregorian one.</p>");
    let _ = write!(
        h,
        concat!(
            "<form id=\"pf\" class=\"year\" hidden><label for=\"py\">Year</label>",
            "<input id=\"py\" name=\"year\" type=\"number\" min=\"1\" max=\"200000\" step=\"1\" value=\"{year}\" inputmode=\"numeric\">",
            "<button type=\"submit\">Show the year</button></form>",
            "<div id=\"pout\" aria-live=\"polite\"><h3>{year}</h3>{table}</div>"
        ),
        year = year,
        table = features::cycle_table(year)
    );
    let _ = write!(
        h,
        "<details class=\"picker\"><summary>Pascha year by year, {} to {}, and the years 40,000 and 41,000</summary><div class=\"shelf\">{}</div></details>",
        year,
        year + 20,
        features::pascha_years(year, year + 20, &[40_000, 41_000])
    );
    h.push_str("<p class=\"meta\">Computed here: Pascha (Meeus, <em>Astronomical Algorithms</em>, ch. 8, Julian computus), the Julian and civil dates (Richards' day-number algorithms), and the movable cycle, whose offsets were checked against orthocal.info for 2026. Taken from orthocal.info: the daily readings, saints, fasts and tones below.</p></section>");
    h
}

/// The lectionary page.
#[must_use]
pub fn lectionary(ctx: &Ctx<'_>) -> String {
    let mut b = String::from("<div class=\"wrap\">");
    b.push_str("<h1>The Lectionary</h1>");
    b.push_str(&paschalion_section(ctx));
    b.push_str("<h2 id=\"daily\">Daily readings</h2>");
    match ctx.lect {
        Some(l) if !l.days.is_empty() => {
            let first = l.days.first().map_or("", |d| d.date.as_str());
            let last = l.days.last().map_or("", |d| d.date.as_str());
            let _ = write!(
                b,
                "<p class=\"sub\">Daily readings and commemorations of the Orthodox Church, {} to {}. Calendar: {}. Each reading opens at its first verse in the King James text.</p><ol class=\"days\">",
                esc(first),
                esc(last),
                esc(&l.calendar)
            );
            for d in &l.days {
                let _ = write!(
                    b,
                    "<li id=\"d-{}\"><span class=\"d\">{}</span>{}</li>",
                    esc(&d.date),
                    esc(&d.date),
                    day_html(ctx, d)
                );
            }
            b.push_str("</ol>");
            let _ = write!(
                b,
                "<p class=\"meta\">Data from <a href=\"https://orthocal.info/\">orthocal.info</a> (MIT License), fetched {}.</p>",
                esc(&l.fetched)
            );
        }
        _ => b.push_str("<p>The lectionary window has not been fetched for this build.</p>"),
    }
    b.push_str("</div>");
    let meta = PageMeta {
        title: "Orthodox Lectionary and Paschalion for Any Year · The Scriptures".to_string(),
        description: "The Orthodox Paschalion for any year, computed exactly, and the daily Epistle and Gospel readings with the saints commemorated, linked to the King James text.".to_string(),
        path: "lectionary/".to_string(),
        og: ctx.art.works.first().and_then(|w| ctx.og_for(w)),
        og_type: "website",
        json_ld: vec![breadcrumb_ld(ctx, &[("The Scriptures".into(), String::new()), ("Lectionary".into(), "lectionary/".into())])],
        section: "lectionary",
        noindex: false,
        preload_image: None,
        extra_script: ctx.wasm.map(features::paschalion_script),
        own_search: false,
        module: None,
    };
    html::page(ctx.cfg, &meta, &b)
}

/// The art index: the plates in canonical order.
#[must_use]
pub fn art_index(ctx: &Ctx<'_>) -> String {
    crate::gallery::page(ctx, "art/")
}

/// The preserved `art/browse/` address: the same gallery, canonical to `art/`.
#[must_use]
pub fn art_directory(ctx: &Ctx<'_>) -> String {
    crate::gallery::page(ctx, "art/browse/")
}

/// The passage quote (first verses, red letters kept) or the note for books outside the 66.
fn art_passage_block(ctx: &Ctx<'_>, w: &Artwork) -> String {
    let mut b = String::new();
    if let Some(start) = w.start() {
        let end = w
            .passage
            .as_deref()
            .and_then(|p| p.split('-').nth(1))
            .and_then(xref::parse_one)
            .unwrap_or(start);
        let mut verses = Vec::new();
        for loc in xref::expand_range(ctx.bible, w.passage.as_deref().unwrap_or(""))
            .iter()
            .take(3)
        {
            verses.push(format!(
                "<sup class=\"vn\">{}</sup>{}",
                loc.verse,
                ctx.verse_html(loc.book, loc.chapter, loc.verse)
            ));
        }
        let more = (end.chapter, end.verse) != (start.chapter, start.verse) && verses.len() == 3;
        let _ = write!(
            b,
            "<blockquote><p>{}{}</p><cite><a href=\"{}\">{}</a> · King James Version</cite></blockquote>",
            verses.join(" "),
            if more {
                " <span class=\"more\">\u{2026}</span>"
            } else {
                ""
            },
            esc(&ctx.passage_href(w).unwrap_or_default()),
            esc(&ctx.passage_label(w))
        );
    } else if let Some(r) = &w.other_ref {
        let _ = write!(
            b,
            "<p class=\"panel\">Illustrates {}. This book is read in the Orthodox Church with the Old Testament but is not among the 66 books of the King James text served here.</p>",
            esc(r)
        );
    }
    b
}

/// The credit and licence facts of a work.
fn art_credit_block(w: &Artwork, artist: &str, dates: &str) -> String {
    let mut b = String::new();
    b.push_str("<h2 id=\"credit\">Credit and licence</h2><dl class=\"facts\">");
    let rows: Vec<(&str, String)> = vec![
        ("Artist", format!("{} ({dates})", esc(artist))),
        ("Work", esc(&w.scene)),
        (
            "Date",
            esc(if w.date.is_empty() {
                "Not stated by the source"
            } else {
                &w.date
            }),
        ),
        ("Medium", esc(&w.medium)),
        (
            "Licence (Commons)",
            format!(
                "{} <span class=\"meta\">(code <code>{}</code>)</span>",
                esc(&w.licence_short),
                esc(&w.licence_code)
            ),
        ),
        ("Usage terms (Commons)", esc(&w.usage_terms)),
        (
            "Source file",
            format!(
                "<a href=\"{}\">{}</a>",
                esc(&w.commons_url),
                esc(&w.commons_file)
            ),
        ),
        (
            "Retrieved",
            esc(w.retrieved.get(..10).unwrap_or(&w.retrieved)),
        ),
    ];
    for (k, v) in rows {
        let _ = write!(b, "<dt>{k}</dt><dd>{v}</dd>");
    }
    b.push_str("</dl>");
    let _ = write!(
        b,
        "<p>This artwork is in the public domain and no copyright is claimed in it. The image files served here carry embedded XMP and IPTC metadata: the credit \u{201c}The Scriptures, Goroshi LLC\u{201d}, the public-domain status of the artwork, the notice for this site's own text and design, and the King James verse it illustrates with Luke 1:37. Suggested credit: <em>{}, {}{}. Via The Scriptures, Goroshi LLC.</em></p>",
        esc(artist),
        esc(&w.scene),
        if w.date.is_empty() {
            String::new()
        } else {
            format!(", {}", esc(&w.date))
        }
    );
    b
}

/// schema.org `VisualArtwork` with its `ImageObject` licence fields.
fn artwork_ld(ctx: &Ctx<'_>, w: &Artwork, artist: &str, path: &str) -> Value {
    let largest = ctx
        .served
        .get(&w.slug)
        .and_then(|s| s.variants.last())
        .map(|(_, _, p)| ctx.cfg.abs(p))
        .unwrap_or_default();
    let licence = w.licence_url.clone().unwrap_or_else(|| {
        if w.pd_mark() {
            "https://creativecommons.org/publicdomain/mark/1.0/".to_string()
        } else {
            w.commons_url.clone()
        }
    });
    json!({
        "@type": "VisualArtwork",
        "name": w.scene,
        "creator": {"@type": "Person", "name": artist},
        "dateCreated": w.date,
        "artMedium": w.medium,
        "url": ctx.cfg.abs(path),
        "about": ctx.passage_label(w),
        "image": {
            "@type": "ImageObject",
            "contentUrl": largest,
            "license": licence,
            "acquireLicensePage": w.commons_url,
            "creditText": format!("{artist}, {}. Via The Scriptures, Goroshi LLC", w.scene),
            "creator": {"@type": "Person", "name": artist},
            "copyrightNotice": "Public domain"
        }
    })
}

/// One artwork's page.
#[must_use]
pub fn art_page(
    ctx: &Ctx<'_>,
    w: &Artwork,
    prev: Option<&Artwork>,
    next: Option<&Artwork>,
) -> String {
    let facts = artist_facts(&w.artist_key);
    let artist = facts.map_or_else(|| w.artist.clone(), |f| f.name.to_string());
    let dates = facts.map_or("", |f| f.dates);
    let path = format!("art/{}/", w.slug);
    let mut b = String::from("<div class=\"wrap\">");
    b.push_str(&crumbs(
        ctx,
        &[
            ("The Scriptures".into(), Some(ctx.cfg.href(""))),
            ("Art".into(), Some(ctx.cfg.href("art/"))),
            (w.scene.clone(), None),
        ],
    ));
    let _ = write!(
        b,
        "<h1>{}</h1><p class=\"sub\">{} · {artist}{}</p>",
        esc(&w.scene),
        esc(&w.medium),
        if w.date.is_empty() {
            String::new()
        } else {
            format!(", {}", esc(&w.date))
        }
    );
    b.push_str(&ctx.plate(w, true, false));
    b.push_str(&art_passage_block(ctx, w));
    b.push_str(&art_credit_block(w, &artist, dates));
    if let Some(n) = &w.note {
        let _ = write!(b, "<p class=\"meta\">Passage note: {}</p>", esc(n));
    }
    if !w.commons_description.is_empty() {
        let _ = write!(
            b,
            "<p class=\"meta\">Commons description: {}</p>",
            esc(&w.commons_description)
        );
    }
    b.push_str(&pager(
        prev.map(|p| (ctx.cfg.href(&format!("art/{}/", p.slug)), p.scene.clone())),
        Some((ctx.cfg.href("art/"), "All the art".to_string())),
        next.map(|n| (ctx.cfg.href(&format!("art/{}/", n.slug)), n.scene.clone())),
    ));
    b.push_str("</div>");
    let served = ctx.served.get(&w.slug);
    let artwork_ld = artwork_ld(ctx, w, &artist, &path);
    let desc = truncate_words(
        &format!(
            "{} by {artist}{}, illustrating {} (KJV). Public domain, from Wikimedia Commons, with credit and licence.",
            w.scene,
            if w.date.is_empty() {
                String::new()
            } else {
                format!(" ({})", w.date)
            },
            ctx.passage_label(w)
        ),
        160,
    );
    let preload = served.and_then(|s| {
        s.variants
            .last()
            .map(|(_, _, p)| (ctx.cfg.href(p), ctx.src_set(s)))
    });
    let meta = PageMeta {
        title: format!("{} · {artist} · The Scriptures", w.scene),
        description: desc,
        path: path.clone(),
        og: ctx.og_for(w),
        og_type: "article",
        json_ld: vec![
            breadcrumb_ld(
                ctx,
                &[
                    ("The Scriptures".into(), String::new()),
                    ("Art".into(), "art/".into()),
                    (w.scene.clone(), path),
                ],
            ),
            artwork_ld,
        ],
        section: "art",
        noindex: false,
        preload_image: preload,
        extra_script: None,
        own_search: false,
        module: None,
    };
    html::page(ctx.cfg, &meta, &b)
}

/// Sources, licences and attribution.
#[must_use]
pub fn about(ctx: &Ctx<'_>) -> String {
    let john_15_5 = ctx.verse_html(42, 15, 5);
    let refused = ctx.art.refused.len();
    let mut b = String::from("<div class=\"wrap\">");
    b.push_str("<h1>Sources and licences</h1>");
    let _ = write!(
        b,
        "<p class=\"lead\">Source and inspiration: <strong>JESUS CHRIST</strong>, who is God. All of it is from Him.</p><blockquote><p>{john_15_5}</p><cite><a href=\"{}\">John 15:5</a></cite></blockquote>",
        esc(&ctx.verse_href(42, 15, 5))
    );
    let _ = write!(
        b,
        concat!(
            "<h2 id=\"copyright\">Copyright</h2>",
            "<p>© {year} Stefano Theofanous and Goroshi LLC. The text written for this site, its design and its code are free and open source, licensed MIT or Apache-2.0 at your option: <a href=\"https://github.com/goroshillc/scriptures\">github.com/goroshillc/scriptures</a>.</p>",
            "<p>No copyright is claimed in the King James text or in the public-domain artworks shown here.</p>",
            "<h2 id=\"licences\">The text</h2>",
            "<p><strong>King James Version</strong> (1769 standard text), public domain. In the United Kingdom, rights in the Authorized Version are vested in the Crown and administered by its patentees; everywhere else it is in the public domain. {verses} verses in {chapters} chapters of 66 books.</p>",
            "<p><strong>Words of Christ in red.</strong> Located from the red-letter markup (<code>\\wj</code>) of the public-domain King James text published by <a href=\"https://ebible.org/kjv/\">eBible.org</a> (courtesy of the Crosswire Bible Society and eBible.org), aligned word by word onto the text served here.</p>",
            "<h2>The cross-references</h2>",
            "<p>{links} cross-references from the <em>Treasury of Scripture Knowledge</em> (public domain), as compiled by <a href=\"https://www.openbible.info/labs/cross-references/\">OpenBible.info</a> and licensed under a <a href=\"https://creativecommons.org/licenses/by/4.0/\">Creative Commons Attribution licence</a>. Under each verse every reference is resolved to its text; ranges longer than {shown} verses show their first {shown} verses and open in full at the link.</p>",
            "<h2>The lectionary</h2>",
            "<p>Daily readings and commemorations from <a href=\"https://orthocal.info/\">orthocal.info</a>, an Orthodox calendar service by Brian Glass (MIT License, © 2022 Brian Glass). Only dates, titles, names and reading references are used.</p>",
            "<h2>The art</h2>",
            "<p>{works} works by Gustave Doré (from his illustrated Bible of 1866 and his paintings), Albrecht Dürer and Titian, fetched from <a href=\"https://commons.wikimedia.org/\">Wikimedia Commons</a>. For every file the licence was read from the Commons API and only public-domain and CC0 files were admitted ({refused} candidate files were refused). Each work's page records its Commons licence verbatim, its source file and the date it was retrieved. No artwork on this site is generated or redrawn. Each plate is the Commons original upload, proven to be the very file that was reviewed by its sha1, then only resized and re-encoded (AVIF, WebP and JPEG, up to 2,560 pixels, and the full scan up to 6,000 pixels for the zoom view). Nothing is cropped, recoloured or sharpened; only the small social-card images are cropped to the card's shape.</p>",
            "<p>Every JPEG served here carries embedded XMP and IPTC metadata: the credit \u{201c}The Scriptures, Goroshi LLC\u{201d}, the artwork's public-domain status (no copyright is claimed in the artwork), this site's own notice, and King James verses, among them Luke 1:37: \u{201c}{luke}\u{201d} The AVIF and WebP files are the same images encoded for speed; each work's page carries its credit.</p>",
            "<h2>Not used</h2>",
            "<p>The Orthodox Study Bible's translation and notes are copyrighted and are not used here. The canon structure and the public-domain sources above are the whole of it.</p>",
            "<h2>Privacy</h2>",
            "<p>No accounts, no analytics, no third-party requests. Your theme choice and your place in the reading are kept in your own browser. If you begin a private record on your profile, it is kept in your browser too (its storage and one cookie for this site only, which no server reads) and in any progress file you download. Nothing is sent to us.</p>"
        ),
        year = ctx.cfg.year(),
        verses = thousands(ctx.bible.total_verses()),
        chapters = thousands(ctx.bible.total_chapters()),
        links = thousands(ctx.xrefs.total_links()),
        shown = XREF_SHOWN_VERSES,
        works = ctx.art.works.len(),
        refused = refused,
        luke = ctx
            .bible
            .verse_text(41, 1, 37)
            .map(|t| esc(&t))
            .unwrap_or_default(),
    );
    b.push_str("<p>Site emblem: the Chi Rho within a halo, Alpha and Omega beside it. The Chi Rho is the U+2627 letterform of <a href=\"https://github.com/psb1558/Junicode-font\">Junicode</a> 2.226 by Peter S. Baker, used under the <a href=\"https://openfontlicense.org/\">SIL Open Font License 1.1</a>, set within one circle.</p>");
    b.push_str("<h2>The Paschalion</h2><p>Pascha for any year is computed with the Julian computus in the form given by Jean Meeus, <em>Astronomical Algorithms</em> (2nd ed., 1998), chapter 8, and dated with the day-number algorithms of E. G. Richards (<em>Explanatory Supplement to the Astronomical Almanac</em>, 3rd ed., 2013, ch. 15). The movable feasts are counted from it; their offsets were checked against orthocal.info for 2026, and Pascha against orthocal.info for 2026, 2027, 2100, 2500, 3000 and 4099. The same Rust code runs in the build and, compiled to WebAssembly, in your browser.</p>");
    b.push_str("<h2>Links</h2><ul><li><a href=\"https://ebible.org/kjv/\">The King James Bible</a> at eBible.org, the public-domain edition whose red-letter markup this site follows</li><li><a href=\"https://orthodoxyinamerica.org/\">Orthodoxy in America</a></li><li><a href=\"https://github.com/goroshillc/scriptures\">The source code of this reader</a> (free and open: MIT or Apache-2.0)</li></ul>");
    if let Some(p) = ctx.psalm {
        b.push_str(&features::non_nobis(ctx, p, true));
    }
    b.push_str("<p class=\"gk center\" lang=\"grc\">ΙϹ ΧϹ ΝΙΚΑ</p></div>");
    let meta = PageMeta {
        title: "Sources, Licences and Attribution · The Scriptures".to_string(),
        description: "Where every part of The Scriptures comes from: the public-domain King James text, the Treasury of Scripture Knowledge (CC BY), orthocal.info (MIT) and public-domain art.".to_string(),
        path: "about/".to_string(),
        og: None,
        og_type: "website",
        json_ld: vec![breadcrumb_ld(ctx, &[("The Scriptures".into(), String::new()), ("About".into(), "about/".into())])],
        section: "about",
        noindex: false,
        preload_image: None,
        extra_script: None,
        own_search: false,
        module: None,
    };
    html::page(ctx.cfg, &meta, &b)
}

/// The not-found page.
#[must_use]
pub fn not_found(ctx: &Ctx<'_>) -> String {
    let ps_119_105 = ctx.verse_html(18, 119, 105);
    let body = format!(
        "<div class=\"wrap\"><h1>Page not found</h1><blockquote><p>{ps_119_105}</p><cite>Psalms 119:105</cite></blockquote><p><a href=\"{}#books\">Choose a book</a> or open <a href=\"{}\">John 1</a>.</p></div>",
        esc(&ctx.cfg.href("")),
        esc(&ctx.cfg.href("john/1/"))
    );
    let meta = PageMeta {
        title: "Not found · The Scriptures".to_string(),
        description: "This page does not exist.".to_string(),
        path: "404.html".to_string(),
        og: None,
        og_type: "website",
        json_ld: Vec::new(),
        section: "",
        noindex: true,
        preload_image: None,
        extra_script: None,
        own_search: false,
        module: None,
    };
    html::page(ctx.cfg, &meta, &body)
}

/// `31102` -> `"31,102"`.
#[must_use]
pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Variant lookup helper for the site builder.
#[must_use]
pub fn variant_path(v: &Variant) -> String {
    format!("art/img/{}", v.file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_html_wraps_spans_and_escapes_everything() {
        let t = "Jesus saith unto him, I am the way <&>.";
        let html = red_html(t, &[(22, 39)]);
        assert_eq!(
            html,
            "Jesus saith unto him, <span class=\"wj\">I am the way &lt;&amp;&gt;.</span>"
        );
    }

    #[test]
    fn the_reading_loops_from_revelation_to_genesis_and_back() -> crate::Result<()> {
        let bible =
            Bible::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/kjv.json"))?;
        // Revelation 22 -> Genesis 1, and Genesis 1 <- Revelation 22.
        assert_eq!(chapter_loop(&bible, 65, 22), ((65, 21), (0, 1)));
        assert_eq!(chapter_loop(&bible, 0, 1), ((65, 22), (0, 2)));
        // Book boundaries inside the canon: Malachi 4 -> Matthew 1, Matthew 1 <- Malachi 4.
        assert_eq!(chapter_loop(&bible, 38, 4).1, (39, 1));
        assert_eq!(chapter_loop(&bible, 39, 1).0, (38, 4));
        Ok(())
    }

    #[test]
    fn red_narrative_quiets_only_the_words_around_his() {
        let t = "Jesus saith unto him, I am the way.";
        assert_eq!(
            red_narrative_html(t, &[(22, 35)]),
            "<span class=\"n\">Jesus saith unto him, </span><span class=\"wj\">I am the way.</span>"
        );
        assert_eq!(
            red_narrative_html("In the beginning", &[]),
            "In the beginning"
        );
    }

    #[test]
    fn thousands_groups_by_three() {
        assert_eq!(thousands(326_308), "326,308");
        assert_eq!(thousands(66), "66");
        assert_eq!(thousands(1_189), "1,189");
    }

    #[test]
    fn truncate_words_ends_on_a_word_with_an_ellipsis() {
        let s = truncate_words("For God so loved the world, that he gave", 20);
        assert_eq!(s, "For God so loved\u{2026}");
    }
}

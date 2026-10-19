//! Source-preserving discovery pages and their original Rust search catalogue.

use crate::{Result, canon, html, render::Ctx, xref};
use scripture_search::Document;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Browser-only DOM, navigation and WebAssembly loading boundary.
pub const BROWSER_GLUE: &str = include_str!("discovery_ui.js");

/// The reader's own browser boundary: verse tools, keys, zoom, the gallery filter, and
/// the private record, whose every rule and number is computed by the Rust engine.
pub const READER_GLUE: &str = include_str!("reader.js");

/// Bind asset URLs to their admitted inputs and exact compiled search engine.
///
/// # Errors
/// Refuses missing inputs or missing compiled search assets.
pub fn asset_version(data: &std::path::Path, cfg: &crate::site::SiteConfig) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for name in [
        "kjv.json",
        "redletter.json",
        "art_manifest.json",
        "ag_child_removed.tsv",
        "content_review.tsv",
    ] {
        let file = data.join(name);
        if !file.exists() && name == "content_review.tsv" {
            continue;
        }
        hash.update(
            std::fs::read(&file)
                .map_err(|error| crate::Error::io("discovery input binding", &file, error))?,
        );
    }
    let assets = cfg.search_assets.as_ref().ok_or_else(|| {
        crate::Error::Usage(
            "build requires --search-assets with qualified Rust WASM bindings".into(),
        )
    })?;
    for name in ["discovery.js", "discovery_bg.wasm"] {
        let file = assets.join(name);
        hash.update(
            std::fs::read(&file)
                .map_err(|error| crate::Error::io("discovery engine binding", &file, error))?,
        );
    }
    hash.update(BROWSER_GLUE.as_bytes());
    hash.update(READER_GLUE.as_bytes());
    hash.update(include_bytes!("discovery.rs"));
    hash.update(cfg.build_date.as_bytes());
    hash.update(cfg.origin.as_bytes());
    hash.update(cfg.base_path.as_bytes());
    Ok(format!("{:x}", hash.finalize()))
}

/// Editorial reading trails. They are navigation aids, not exhaustive doctrine.
pub const TOPICS: &[(&str, &str, &str)] = &[
    (
        "hope",
        "Hope & perseverance",
        "Rom.8.18-Rom.8.39;Ps.42.1-Ps.42.11;John.16.33",
    ),
    (
        "love",
        "Love & mercy",
        "John.3.16-John.3.21;1Cor.13.1-1Cor.13.13;Luke.10.25-Luke.10.37",
    ),
    (
        "prayer",
        "Prayer",
        "Matt.6.5-Matt.6.15;Luke.11.1-Luke.11.13;Ps.23.1-Ps.23.6",
    ),
    (
        "peace",
        "Peace & courage",
        "John.14.1-John.14.31;Phil.4.4-Phil.4.9;Ps.27.1-Ps.27.14",
    ),
    (
        "wisdom",
        "Wisdom & discernment",
        "Prov.3.1-Prov.3.12;Jas.1.2-Jas.1.8;Matt.7.15-Matt.7.29",
    ),
    (
        "forgiveness",
        "Forgiveness & repentance",
        "Luke.15.1-Luke.15.32;Ps.51.1-Ps.51.19;Matt.18.21-Matt.18.35",
    ),
    (
        "resurrection",
        "Resurrection & eternal life",
        "John.11.1-John.11.44;John.20.1-John.20.31;Ezek.37.1-Ezek.37.14;1Cor.15.1-1Cor.15.58",
    ),
    (
        "creation",
        "Creation",
        "Gen.1.1-Gen.2.3;Ps.104.1-Ps.104.35;John.1.1-John.1.18",
    ),
    (
        "faith",
        "Faith & trust",
        "Heb.11.1-Heb.11.40;Mark.9.14-Mark.9.29;Matt.14.22-Matt.14.33",
    ),
    (
        "justice",
        "Justice & compassion",
        "Mic.6.6-Mic.6.8;Matt.25.31-Matt.25.46;Isa.58.1-Isa.58.14",
    ),
];

fn topic_index(bible: &crate::bible::Bible) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (slug, _, passages) in TOPICS {
        for passage in passages.split(';') {
            for loc in xref::expand_range(bible, passage) {
                map.entry(xref::osis_key(loc.book, loc.chapter, loc.verse))
                    .or_default()
                    .push((*slug).into());
            }
        }
    }
    map
}

/// One catalogue document per verse, in canon order, with its facets.
fn verse_documents(ctx: &Ctx<'_>, topics: &BTreeMap<String, Vec<String>>) -> Vec<Document> {
    // Every verse inside the passage of an admitted work, for the "with art" filter.
    let with_art: std::collections::BTreeSet<String> = ctx
        .art
        .works
        .iter()
        .filter_map(|work| work.passage.as_deref())
        .flat_map(|passage| xref::expand_range(ctx.bible, passage))
        .map(|loc| xref::osis_key(loc.book, loc.chapter, loc.verse))
        .collect();
    let mut docs = Vec::new();
    for (book, info) in canon::BOOKS.iter().enumerate() {
        for chapter in 1..=ctx.bible.chapter_count(book) {
            for verse in 1..=ctx.bible.verse_count(book, chapter) {
                let key = xref::osis_key(book, chapter, verse);
                let Some(text) = ctx.bible.verse_text(book, chapter, verse) else {
                    continue;
                };
                // The words of Christ as character ranges, for red-letter-only search.
                let red: Vec<[usize; 2]> = ctx
                    .red
                    .spans(&key)
                    .iter()
                    .filter_map(|&(start, end)| {
                        Some([
                            text.get(..start)?.chars().count(),
                            text.get(..end)?.chars().count(),
                        ])
                    })
                    .filter(|[start, end]| start < end)
                    .collect();
                docs.push(Document {
                    id: docs.len(),
                    kind: "verse".into(),
                    title: format!("{} {chapter}:{verse}", info.name),
                    text,
                    href: ctx.verse_href_pub(book, chapter, verse),
                    book: info.name.into(),
                    testament: if info.testament == canon::Testament::Old {
                        "Old Testament"
                    } else {
                        "New Testament"
                    }
                    .into(),
                    artist: String::new(),
                    topics: topics.get(&key).cloned().unwrap_or_default(),
                    red_letter: !red.is_empty(),
                    image: None,
                    art: with_art.contains(&key),
                    red,
                });
            }
        }
    }
    docs
}

/// Project admitted data into a discovery index, preserving the source verse text.
///
/// # Errors
/// Refuses a malformed catalogue rather than publishing unsafe links.
pub fn catalogue(ctx: &Ctx<'_>) -> Result<Vec<Document>> {
    let topics = topic_index(ctx.bible);
    let mut docs = verse_documents(ctx, &topics);
    let mut works: Vec<_> = ctx.art.works.iter().collect();
    works.sort_by_key(|work| {
        work.start()
            .map_or((usize::MAX, 0, 0), |loc| (loc.book, loc.chapter, loc.verse))
    });
    for work in works {
        let info = work.start().and_then(|loc| canon::book(loc.book));
        let mut tags = Vec::new();
        if let Some(passage) = &work.passage {
            for loc in xref::expand_range(ctx.bible, passage) {
                for tag in topics
                    .get(&xref::osis_key(loc.book, loc.chapter, loc.verse))
                    .into_iter()
                    .flatten()
                {
                    if !tags.contains(tag) {
                        tags.push(tag.clone());
                    }
                }
            }
        }
        let image = ctx
            .served
            .get(&work.slug)
            .and_then(|served| {
                served
                    .variants
                    .iter()
                    .find(|(width, _, _)| *width >= 500)
                    .or_else(|| served.variants.last())
            })
            .map(|(_, _, path)| ctx.cfg.href(path));
        docs.push(Document {
            id: docs.len(),
            kind: "art".into(),
            title: work.scene.clone(),
            text: format!(
                "{} · {} · {}{}",
                work.artist,
                work.date,
                work.medium,
                work.passage
                    .as_ref()
                    .map_or_else(String::new, |p| format!(" · {}", xref::human(ctx.bible, p)))
            ),
            href: ctx.cfg.href(&format!("art/{}/", work.slug)),
            book: info.map_or("Other writings", |b| b.name).into(),
            testament: info
                .map_or("Other writings", |b| {
                    if b.testament == canon::Testament::Old {
                        "Old Testament"
                    } else {
                        "New Testament"
                    }
                })
                .into(),
            artist: work.artist.clone(),
            topics: tags,
            red_letter: false,
            image,
            art: true,
            red: Vec::new(),
        });
    }
    scripture_search::Index::new(docs.clone()).map_err(crate::Error::Data)?;
    Ok(docs)
}

fn select(name: &str, label: &str, values: &[(&str, &str)]) -> String {
    let mut s = format!(
        "<label class=\"facet\">{}<select name=\"{}\"><option value=\"\">All</option>",
        html::esc(label),
        name
    );
    for (value, text) in values {
        let _ = write!(
            s,
            "<option value=\"{}\">{}</option>",
            html::esc(value),
            html::esc(text)
        );
    }
    s.push_str("</select></label>");
    s
}

/// The filter controls of the discovery page: kind, testament, book, artist, trail.
fn filters(ctx: &Ctx<'_>) -> String {
    let mut out = String::new();
    out.push_str(&select(
        "kind",
        "Look in",
        &[("verse", "Scripture"), ("art", "Artwork")],
    ));
    out.push_str("<fieldset class=\"seg\"><legend>Testament</legend>");
    for (value, label) in [
        ("", "Whole Bible"),
        ("Old Testament", "Old Testament"),
        ("New Testament", "New Testament"),
        ("Other writings", "Other writings (art)"),
    ] {
        let _ = write!(
            out,
            "<label><input type=\"radio\" name=\"testament\" value=\"{}\"{}> {}</label>",
            html::esc(value),
            if value.is_empty() { " checked" } else { "" },
            html::esc(label)
        );
    }
    out.push_str("</fieldset>");
    out.push_str(
        "<label class=\"facet\">Book<select name=\"book\"><option value=\"\">All 66 books</option>",
    );
    for (testament, label) in [
        (canon::Testament::Old, "Old Testament"),
        (canon::Testament::New, "New Testament"),
    ] {
        let _ = write!(out, "<optgroup label=\"{label}\">");
        for book in canon::BOOKS.iter().filter(|b| b.testament == testament) {
            let _ = write!(
                out,
                "<option value=\"{0}\">{0}</option>",
                html::esc(book.name)
            );
        }
        out.push_str("</optgroup>");
    }
    out.push_str("</select></label>");
    let mut artists: Vec<_> = ctx
        .art
        .works
        .iter()
        .map(|work| work.artist.as_str())
        .collect();
    artists.sort_unstable();
    artists.dedup();
    out.push_str(&select(
        "artist",
        "Artist",
        &artists
            .iter()
            .map(|name| (*name, *name))
            .collect::<Vec<_>>(),
    ));
    out.push_str(&select(
        "topic",
        "Reading trail",
        &TOPICS
            .iter()
            .map(|(slug, label, _)| (*slug, *label))
            .collect::<Vec<_>>(),
    ));
    out
}

/// A unified discovery interface, optionally starting in the artwork collection.
#[must_use]
pub fn page(ctx: &Ctx<'_>, art: bool) -> String {
    let base = ctx.cfg.href("");
    let path = if art { "art/" } else { "search/" };
    let title = if art {
        "Explore the art"
    } else {
        "Find your way through Scripture"
    };
    let sub = if art {
        "Search by scene, artist or passage. Follow each work back to its source and the story it illustrates."
    } else {
        "A remembered phrase, a passage, a question. Start there, then read the words in their full chapter."
    };
    let mut body = format!(
        r#"<div class="wrap discovery" id="discovery" data-base="{}" data-default-kind="{}"><div class="discover-head"><p class="eyebrow">THE SCRIPTURES / DISCOVERY</p><h1>{title}</h1><p class="sub">{sub}</p></div><form id="discover-form" class="discovery-layout" action="{}"><div class="searchbar"><label for="discover-query" class="sr">Search Scripture and artwork</label><input id="discover-query" name="q" type="search" maxlength="256" placeholder="Try John 3:16, “love one another”, or Doré" autocomplete="off" enterkeyhint="search" aria-describedby="result-count"><button type="submit" class="discover-button">Search</button></div><aside class="filters"><h2>Refine your search</h2>"#,
        html::esc(&base),
        if art { "art" } else { "" },
        html::esc(&ctx.cfg.href(path))
    );
    body.push_str(&filters(ctx));
    body.push_str(r#"<fieldset class="seg"><legend>Match</legend><label><input type="radio" name="mode" value="all" checked> All of the words</label><label><input type="radio" name="mode" value="any"> Any of the words</label><label><input type="radio" name="mode" value="phrase"> The exact phrase</label></fieldset><label class="check"><input type="checkbox" name="red_letter"> Words of Christ only</label><label class="check"><input type="checkbox" name="with_art"> Verses with art</label><label class="check"><input type="checkbox" name="fuzzy"> Allow near spellings (marked)</label><label class="check"><input type="checkbox" name="exact_spelling"> Only the spelling typed (not shew for show)</label><button type="button" id="clear-filters" class="quiet-button">Clear search & filters</button><p class="filter-note">Type a reference such as John 3:16, Jn 3 or Ps 23 to open the passage. King James spellings are searched too: show finds shew, says finds saith. Reading trails are selected starting points, not exhaustive lists.</p><p class="keys">Keys: <kbd>/</kbd> search · <kbd>Esc</kbd> clear · <kbd>Tab</kbd> to the results</p></aside><section class="discover-main" aria-label="Search results"><div id="jump"></div><div class="topic-chips" aria-label="Start a reading trail">"#);
    for (slug, label, _) in TOPICS {
        let _ = write!(
            body,
            "<a class=\"topic-chip\" href=\"{}?topic={slug}&amp;kind=verse\" data-topic=\"{slug}\">{}</a>",
            html::esc(&ctx.cfg.href("search/")),
            html::esc(label)
        );
    }
    body.push_str(r#"</div><div class="results-toolbar"><p id="result-count" role="status" aria-live="polite">Loading the search catalogue…</p><label>Order <select name="sort"><option value="relevance">Best match</option><option value="canonical">Scripture order</option><option value="title">Title A–Z</option></select></label></div><div id="search-notes"></div><div id="book-counts" class="book-counts" aria-label="Matches by book"></div><div id="active-filters" class="active-filters"></div><div id="search-results" class="search-results"></div><div id="empty-results" hidden><h2>No matches yet</h2><p>Try fewer words, another spelling, or clear a filter. Reading trails can help when you know the subject but not the wording.</p><button type="button" id="empty-clear" class="discover-button">Clear all filters</button></div><nav class="result-pages" aria-label="Result pages"><button type="button" id="previous-results">Previous</button><span id="page-label"></span><button type="button" id="next-results">Next</button></nav><p class="filter-note">Search runs on your device. Your search words are not sent to a search service.</p></section></form>"#);
    let _ = write!(
        body,
        "<noscript><p>Interactive search needs JavaScript and WebAssembly. You can still <a href=\"{}\">browse every book</a> or <a href=\"{}\">browse artwork by book</a>.</p></noscript></div><script type=\"module\" src=\"{}\"></script>",
        html::esc(&ctx.cfg.href("")),
        html::esc(&ctx.cfg.href("art/browse/")),
        html::esc(
            &ctx.cfg
                .href(&format!("discovery/ui.js?v={}", ctx.cfg.discovery_version))
        )
    );
    let body = body.replace(
        "id=\"discovery\"",
        &format!(
            "id=\"discovery\" data-version=\"{}\"",
            html::esc(&ctx.cfg.discovery_version)
        ),
    );
    html::page(
        ctx.cfg,
        &html::PageMeta {
            title: format!("{title} | The Scriptures"),
            description: sub.into(),
            path: path.into(),
            section: if art { "art" } else { "search" },
            og_type: "website",
            own_search: true,
            ..html::PageMeta::default()
        },
        &body,
    )
}

/// Direct connections to the authoritative parish directories, without copying their map.
#[must_use]
pub fn parishes(ctx: &Ctx<'_>) -> String {
    let body = r#"<div class="wrap"><div class="discover-head"><p class="eyebrow">FROM READING TO COMMUNITY</p><h1>Find an Orthodox parish</h1><p class="sub">Explore a church near you through the directories maintained by the Church.</p></div><div class="parish-grid"><article class="parish-card"><span class="eyebrow">OFFICIAL DIRECTORY</span><h2>Orthodox Church in America</h2><p>Browse OCA parishes by state or province, diocese and deanery, or search by name.</p><a class="discover-button" href="https://www.oca.org/parishes">Open the OCA parish directory ↗</a><div class="parish-shortcuts"><a href="https://www.oca.org/parishes/state/IL">Illinois</a><a href="https://www.oca.org/parishes/state/IN">Indiana</a><a href="https://www.oca.org/parishes/state/WI">Wisconsin</a><a href="https://www.oca.org/parishes">All locations</a></div></article><article class="parish-card"><span class="eyebrow">MAP & PROXIMITY SEARCH</span><h2>Across Orthodox jurisdictions</h2><p>Use the Assembly of Canonical Orthodox Bishops directory to find parishes by proximity or jurisdiction.</p><a class="discover-button" href="https://www.assemblyofbishops.org/directories/parishes">Open the Orthodox parish finder ↗</a></article></div><section class="panel"><h2>Before you visit</h2><p>Follow the parish website for current service times and contact details. Listings and schedules can change; the parish is the source for its current information.</p><p>These links open the official directories. This site does not request your location or load an external map in the background.</p></section></div>"#;
    let body = body.replacen("<div class=\"parish-grid\">", concat!(
        "<section class=\"parish-card\"><span class=\"eyebrow\">INTERACTIVE CHURCH LOCATOR</span><h2>Orthodoxy in America</h2>",
        "<p>Open the map to search nearby Orthodox churches and narrow the listings with its filters.</p>",
        "<a class=\"discover-button\" href=\"https://orthodoxyinamerica.org/\">Explore the parish map ↗</a>",
        "<p class=\"filter-note\">Locator maintained by Orthodox Web Solutions. Confirm service times with the parish before visiting.</p></section><div class=\"parish-grid\">"
    ), 1).replace("These links open the official directories.", "These links open the locator and official directories.");
    html::page(ctx.cfg, &html::PageMeta { title: "Find an Orthodox parish | The Scriptures".into(), description: "Orthodoxy in America interactive parish map, plus official OCA and Assembly of Bishops directories.".into(), path: "parishes/".into(), section: "parishes", og_type: "website", ..html::PageMeta::default() }, &body)
}

/// Emit an offline browser preview with the exact built catalogue and WASM bytes.
///
/// # Errors
/// Refuses missing build assets or any existing preview destination.
pub fn preview(build: &std::path::Path, output: &std::path::Path, base: &str) -> Result<()> {
    use std::io::Write as _;
    let read = |path: &str| {
        std::fs::read(build.join(path))
            .map_err(|error| crate::Error::io("preview input", build.join(path), error))
    };
    let raw = read("search/index.html")?;
    let mut page =
        String::from_utf8(raw).map_err(|_| crate::Error::Data("Preview HTML encoding".into()))?;
    let json = String::from_utf8(read("discovery/catalogue.json")?)
        .map_err(|_| crate::Error::Data("Preview catalogue encoding".into()))?
        .replace('<', "\\u003c");
    let binding = html::base64(&read("discovery/discovery.js")?);
    let wasm = html::base64(&read("discovery/discovery_bg.wasm")?);
    let root = build
        .canonicalize()
        .map_err(|error| crate::Error::io("preview root", build, error))?;
    page = page.replace(
        "id=\"discovery\"",
        &format!(
            "id=\"discovery\" data-preview-root=\"file://{}/\"",
            html::esc(&root.display().to_string())
        ),
    );
    let version = page
        .split_once("data-version=\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(value, _)| value)
        .ok_or_else(|| crate::Error::Data("Preview asset version missing".into()))?;
    let script = format!(
        "<script type=\"module\" src=\"{}discovery/ui.js?v={version}\"></script>",
        html::esc(base)
    );
    let inline = format!(
        "<script id=\"inline-catalogue\" type=\"application/json\">{json}</script><script id=\"inline-binding\" type=\"text/plain\">{binding}</script><script id=\"inline-wasm\" type=\"text/plain\">{wasm}</script><script type=\"module\">{BROWSER_GLUE}</script>"
    );
    if !page.contains(&script) {
        return Err(crate::Error::Data("Preview module entry missing".into()));
    }
    page = page.replace(&script, &inline);
    let theme = format!(
        "<script src=\"{}{}\"></script>",
        html::esc(base),
        html::script_asset(html::JS)
    );
    page = page.replace(&theme, &format!("<script>{}</script>", html::JS));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| crate::Error::io("new preview", output, error))?;
    file.write_all(page.as_bytes())
        .map_err(|error| crate::Error::io("write preview", output, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reading_trail_reference_resolves_to_retained_kjv_text() -> Result<()> {
        let bible = crate::bible::Bible::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/kjv.json"),
        )?;
        for (_, _, passages) in TOPICS {
            for passage in passages.split(';') {
                let verses = xref::expand_range(&bible, passage);
                assert!(
                    !verses.is_empty(),
                    "unresolved reading-trail reference: {passage}"
                );
                assert!(
                    verses
                        .iter()
                        .all(|loc| bible.verse_text(loc.book, loc.chapter, loc.verse).is_some())
                );
            }
        }
        let index = topic_index(&bible);
        assert!(
            index
                .get("John.3.16")
                .is_some_and(|tags| tags.iter().any(|tag| tag == "love"))
        );
        assert!(
            index
                .get("Ezek.37.1")
                .is_some_and(|tags| tags.iter().any(|tag| tag == "resurrection"))
        );
        Ok(())
    }
}

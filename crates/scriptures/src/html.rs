//! HTML primitives: escaping, the page shell, the stylesheet and the one small script.
//!
//! > *"Thy word is a lamp unto my feet, and a light unto my path."* - Psalms 119:105, KJV
//!
//! WHY inline CSS and no framework: every page must paint on the first round trip with
//! no required JavaScript. The stylesheet is identical on every page, so it is inlined
//! (one fewer render-blocking request) and pinned by hash in the CSP. Scripts are
//! progressive enhancement only; with them off, every page reads, links and expands.
//!
//! Palette: black,
//! white and greys; the only colour is the deep red of the words of Christ. White
//! ground is the default for every visitor; the switch to dark is remembered per
//! visitor. The unifying mark is the halo, one circle around the Chi Rho (the shared
//! `chi-rho` emblem: Junicode's U+2627 letterform, SIL OFL 1.1).

use crate::site::SiteConfig;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// Escapes text for element content and double-quoted attributes.
#[must_use]
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Escapes a string for embedding inside a JSON-LD `<script>` (no `</script>` breakout).
#[must_use]
pub fn json_ld(v: &serde_json::Value) -> String {
    serde_json::to_string(v)
        .unwrap_or_default()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

/// The site stylesheet: the one design system for every page (`src/reader.css`).
///
/// WHY one file: one unified, centralized design - a single set of type,
/// spacing and colour tokens (black, white, greys, and the deep red of the words of
/// Christ only) used by the reader, search, gallery, red-letter index and profile alike.
/// It is still inlined into each page so the first paint needs no extra request.
pub const CSS: &str = include_str!("reader.css");

/// The one script: theme toggle (light by default, dark remembered per visitor), the
/// `/` key that puts the cursor in the search bar, and the lectionary's "today" mark.
/// Optional by design.
pub const JS: &str = r"(function(){var d=document.documentElement,k='theme';try{if(localStorage.getItem(k)==='dark')d.setAttribute('data-theme','dark')}catch(e){}
function cur(){return d.getAttribute('data-theme')==='dark'?'dark':'light'}
document.addEventListener('DOMContentLoaded',function(){var b=document.getElementById('theme-toggle');if(b){var lab=function(){var c=cur();b.textContent=c==='dark'?'Light':'Dark';b.setAttribute('aria-label','Switch to '+(c==='dark'?'light':'dark')+' theme');b.setAttribute('aria-pressed',c==='dark'?'true':'false')};b.hidden=false;lab();b.addEventListener('click',function(){var n=cur()==='dark'?'light':'dark';if(n==='dark')d.setAttribute('data-theme','dark');else d.removeAttribute('data-theme');try{localStorage.setItem(k,n)}catch(e){}lab()})}
document.addEventListener('keydown',function(e){if(e.key!=='/'||e.ctrlKey||e.metaKey||e.altKey)return;var t=e.target,g=t&&t.tagName;if(g==='INPUT'||g==='TEXTAREA'||g==='SELECT'||(t&&t.isContentEditable))return;var q=document.getElementById('discover-query')||document.getElementById('hq');if(q){e.preventDefault();q.focus();q.select()}});
var n=new Date(),iso=n.getFullYear()+'-'+('0'+(n.getMonth()+1)).slice(-2)+'-'+('0'+n.getDate()).slice(-2),t=document.getElementById('d-'+iso);if(t){t.className+=' today';var m=document.createElement('strong');m.textContent=' Today';t.querySelector('.d').appendChild(m)}})})();";

/// The stylesheet as served: the reading face's `@font-face` rules (when the build ships
/// the font files) followed by [`CSS`].
///
/// WHY generated: the font URLs carry the site's base path and each file's content
/// hash, which a constant cannot know; the CSP pins whatever this returns by hash.
#[must_use]
pub fn stylesheet(cfg: &SiteConfig) -> String {
    let mut css = String::new();
    for (style, path) in [("normal", &cfg.font_regular), ("italic", &cfg.font_italic)] {
        if let Some(path) = path {
            let _ = writeln!(
                css,
                "@font-face{{font-family:\"EB Garamond\";src:url(\"{}\") format(\"woff\");font-style:{style};font-weight:400 800;font-display:swap}}",
                cfg.href(path)
            );
        }
    }
    css.push_str(CSS);
    css
}

/// `'sha256-…'` CSP source for an inline block.
#[must_use]
pub fn csp_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    format!("'sha256-{}'", base64(&digest))
}

/// Standard base64 (RFC 4648) with padding, for CSP hashes.
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk.first().copied().unwrap_or(0);
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let idx = |shift: u32| usize::try_from((n >> shift) & 63).unwrap_or(0);
        out.push(char::from(T.get(idx(18)).copied().unwrap_or(b'A')));
        out.push(char::from(T.get(idx(12)).copied().unwrap_or(b'A')));
        out.push(if chunk.len() > 1 {
            char::from(T.get(idx(6)).copied().unwrap_or(b'A'))
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(T.get(idx(0)).copied().unwrap_or(b'A'))
        } else {
            '='
        });
    }
    out
}

/// Social image for a page.
#[derive(Debug, Clone)]
pub struct OgImage {
    /// Absolute URL.
    pub url: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Alt text.
    pub alt: String,
}

/// Everything the `<head>` needs for one page.
#[derive(Debug, Clone, Default)]
pub struct PageMeta {
    /// `<title>`.
    pub title: String,
    /// Meta description (aim for 120-160 characters).
    pub description: String,
    /// Site-relative path without a leading slash (`"john/3/"`, `""` for home).
    pub path: String,
    /// Social image.
    pub og: Option<OgImage>,
    /// `og:type`.
    pub og_type: &'static str,
    /// JSON-LD graph nodes.
    pub json_ld: Vec<serde_json::Value>,
    /// Which top-nav item is current.
    pub section: &'static str,
    /// Exclude from indexing (the 404 page).
    pub noindex: bool,
    /// A URL to preload as the likely LCP image.
    pub preload_image: Option<(String, String)>,
    /// A page-specific inline script (pinned in the CSP by hash like the shared one).
    pub extra_script: Option<String>,
    /// The page has its own search form, so the header search bar is left out.
    pub own_search: bool,
    /// A module script for the page (site-relative path with its version query).
    pub module: Option<String>,
}

/// Wraps a page body in the full document: head, header, footer.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn page(cfg: &SiteConfig, m: &PageMeta, body: &str) -> String {
    let canonical = cfg.abs(&m.path);
    let mut h = String::with_capacity(body.len() + 9000);
    let _ = write!(
        h,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><meta name=\"description\" content=\"{}\">",
        esc(&m.title),
        esc(&m.description)
    );
    if m.noindex {
        h.push_str("<meta name=\"robots\" content=\"noindex\">");
    } else {
        let _ = write!(h, "<link rel=\"canonical\" href=\"{}\">", esc(&canonical));
    }
    h.push_str("<meta name=\"color-scheme\" content=\"light\"><meta name=\"theme-color\" content=\"#ffffff\">");
    let _ = write!(h, "<link rel=\"icon\" href=\"{}\">", esc(&cfg.icon_href()));
    let _ = write!(
        h,
        "<link rel=\"sitemap\" type=\"application/xml\" href=\"{}\">",
        esc(&cfg.href("sitemap.xml"))
    );
    if let Some((href, srcset)) = &m.preload_image {
        let _ = write!(
            h,
            "<link rel=\"preload\" as=\"image\" href=\"{}\" imagesrcset=\"{}\" imagesizes=\"(max-width: 62.5rem) calc(100vw - 2.5rem), 60rem\" fetchpriority=\"high\">",
            esc(href),
            esc(srcset)
        );
    }
    let og_type = if m.og_type.is_empty() {
        "website"
    } else {
        m.og_type
    };
    let _ = write!(
        h,
        "<meta property=\"og:site_name\" content=\"The Scriptures\"><meta property=\"og:locale\" content=\"en_US\"><meta property=\"og:type\" content=\"{}\"><meta property=\"og:title\" content=\"{}\"><meta property=\"og:description\" content=\"{}\"><meta property=\"og:url\" content=\"{}\">",
        og_type,
        esc(&m.title),
        esc(&m.description),
        esc(&canonical)
    );
    if let Some(og) = &m.og {
        let _ = write!(
            h,
            "<meta property=\"og:image\" content=\"{u}\"><meta property=\"og:image:type\" content=\"image/jpeg\"><meta property=\"og:image:width\" content=\"{w}\"><meta property=\"og:image:height\" content=\"{ht}\"><meta property=\"og:image:alt\" content=\"{a}\"><meta name=\"twitter:card\" content=\"summary_large_image\"><meta name=\"twitter:image\" content=\"{u}\"><meta name=\"twitter:image:alt\" content=\"{a}\">",
            u = esc(&og.url),
            w = og.width,
            ht = og.height,
            a = esc(&og.alt)
        );
    } else {
        h.push_str("<meta name=\"twitter:card\" content=\"summary\">");
    }
    let _ = write!(
        h,
        "<meta name=\"twitter:title\" content=\"{}\"><meta name=\"twitter:description\" content=\"{}\">",
        esc(&m.title),
        esc(&m.description)
    );
    if !m.json_ld.is_empty() {
        let graph = serde_json::json!({"@context": "https://schema.org", "@graph": m.json_ld});
        let _ = write!(
            h,
            "<script type=\"application/ld+json\">{}</script>",
            json_ld(&graph)
        );
    }
    if let Some(font) = &cfg.font_regular {
        let _ = write!(
            h,
            "<link rel=\"preload\" as=\"font\" type=\"font/woff\" href=\"{}\" crossorigin>",
            esc(&cfg.href(font))
        );
    }
    let _ = write!(
        h,
        "<style>{}</style><script src=\"{}\"></script>",
        stylesheet(cfg),
        esc(&cfg.href(&script_asset(JS)))
    );
    if let Some(module) = &m.module {
        let _ = write!(
            h,
            "<script type=\"module\" src=\"{}\"></script>",
            esc(&cfg.href(module))
        );
    }
    // The reader's own enhancements (verse tools, keys, zoom, gallery filter, the private
    // record): one deferred module on every page, never needed to read.
    let _ = write!(
        h,
        "<script type=\"module\" src=\"{}\"></script>",
        esc(&cfg.href(&format!("discovery/reader.js?v={}", cfg.discovery_version)))
    );
    if let Some(extra) = &m.extra_script {
        let _ = write!(
            h,
            "<script src=\"{}\"></script>",
            esc(&cfg.href(&script_asset(extra)))
        );
    }
    h.push_str("</head><body>");
    h.push_str(&header(cfg, m.section, m.own_search));
    let _ = write!(h, "<main id=\"main\">{body}</main>");
    let _ = write!(h, "{}</body></html>", footer(cfg));
    h
}

/// Content-addressed script path compatible with the host's external-only policy.
#[must_use]
pub fn script_asset(script: &str) -> String {
    let hash = crate::site::sha256_hex(script.as_bytes());
    format!("discovery/script-{}.js", hash.get(..16).unwrap_or(&hash))
}

/// The emblem: the Chi Rho within its halo, drawn in the current text colour.
///
/// The letters are U+2627 from Junicode 2.226 (SIL OFL 1.1) and the markup is the shared
/// `chi-rho` crate's, so every page, the favicon and the app icon draw exactly the same
/// emblem.
#[must_use]
pub fn emblem(class: &str) -> String {
    format!(
        "<span class=\"halo {}\" aria-hidden=\"true\">{}</span>",
        esc(class),
        chi_rho::inline("")
    )
}

/// The header's sections, in order: one consistent header on every page.
pub const NAV: [(&str, &str, &str); 7] = [
    ("books", "", "Books"),
    ("search", "search/", "Search"),
    ("red-letter", "red-letter/", "Red Letter"),
    ("art", "art/", "Dor\u{e9}"),
    ("prayer", "prayer/", "Prayer"),
    ("profile", "profile/", "Profile"),
    ("lectionary", "lectionary/", "Lectionary"),
];

fn header(cfg: &SiteConfig, section: &str, own_search: bool) -> String {
    let mut h = String::new();
    h.push_str("<a class=\"skip\" href=\"#main\">Skip to the text</a>");
    let _ = write!(
        h,
        "<header class=\"site\"><div class=\"bar\"><a class=\"brand\" href=\"{}\">{}<span>The Scriptures</span></a><nav class=\"top\" aria-label=\"Site\"><ul>",
        esc(&cfg.href("")),
        emblem("")
    );
    for (key, path, label) in NAV {
        let href = if key == "books" {
            format!("{}#books", cfg.href(""))
        } else {
            cfg.href(path)
        };
        let cur = if section == key {
            " aria-current=\"page\""
        } else {
            ""
        };
        let _ = write!(h, "<li><a href=\"{}\"{cur}>{label}</a></li>", esc(&href));
    }
    h.push_str("</ul></nav>");
    if !own_search {
        // A plain GET form: it works with scripting off, and the search page reads `q`.
        let _ = write!(
            h,
            "<form class=\"hsearch\" role=\"search\" action=\"{}\"><label class=\"sr\" for=\"hq\">Search the Scriptures, or type a reference</label><input id=\"hq\" name=\"q\" type=\"search\" maxlength=\"256\" placeholder=\"Search, or John 3:16\" autocomplete=\"off\" enterkeyhint=\"search\"><kbd title=\"Press / to search\">/</kbd><button type=\"submit\">Search</button></form>",
            esc(&cfg.href("search/"))
        );
    }
    h.push_str("<button id=\"theme-toggle\" type=\"button\" aria-pressed=\"false\" hidden>Dark</button></div></header>");
    h
}

fn footer(cfg: &SiteConfig) -> String {
    // Alpha, the Chi Rho in its halo, Omega: the only Christian symbols the site uses.
    format!(
        concat!(
            "<footer class=\"site\"><div class=\"wrap\">",
            "<p class=\"seal\" role=\"img\" aria-label=\"Alpha, Chi Rho, Omega\"><span aria-hidden=\"true\">\u{391}</span>{emblem}<span aria-hidden=\"true\">\u{3a9}</span></p><p class=\"gk\" lang=\"grc\">\u{399}\u{3f9} \u{3a7}\u{3f9} \u{39d}\u{399}\u{39a}\u{391}</p>",
            "<p class=\"src\">Source and inspiration: <strong>JESUS CHRIST</strong>, who is God; all of it is from Him (John 15:5).</p>",
            "<p class=\"links\"><a href=\"{about}\">Sources and licences</a><a href=\"{parishes}\">Find a parish</a><a href=\"https://ebible.org/kjv/\">The King James Bible (eBible.org)</a><a href=\"https://github.com/goroshillc/scriptures\">Source code (free and open)</a></p>",
            "<p>\u{a9} {year} Stefano Theofanous and Goroshi LLC. Free and open source: the code, site text and design are licensed MIT or Apache-2.0. ",
            "The King James text and the master art are in the public domain; no copyright is claimed in them.</p>",
            "<p>Cross-references from the Treasury of Scripture Knowledge via <a href=\"https://www.openbible.info/labs/cross-references/\">OpenBible.info</a>, licensed <a href=\"https://creativecommons.org/licenses/by/4.0/\">CC BY 4.0</a>. ",
            "Lectionary data from <a href=\"https://orthocal.info/\">orthocal.info</a> (MIT License). ",
            "Words of Christ marked from the <a href=\"https://ebible.org/kjv/\">eBible.org</a> red-letter KJV.</p>",
            "<p>No accounts, no tracking. Your theme, place and private record stay in this browser.</p>",
            "</div></footer>"
        ),
        emblem = emblem("halo-lg"),
        year = cfg.year(),
        about = esc(&cfg.href("about/")),
        parishes = esc(&cfg.href("parishes/"))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_neutralises_markup_and_quotes() {
        assert_eq!(
            esc("<a href=\"x\">'&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;"
        );
    }

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}

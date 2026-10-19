//! The reader's front-of-house pieces: the book selector, the featured chapter, the
//! Non Nobis motto and the Paschalion for any year.
//!
//! > *"Jesus said unto her, I am the resurrection, and the life: he that believeth in
//! > me, though he were dead, yet shall he live:"* - John 11:25, KJV
//!
//! WHY a module of its own: these parts were each asked for by name - the selector on
//! the page itself with a strong hover, Ezekiel 37 because "Jesus can raise people from
//! the dead", the Psalm 115 motto exactly as its author wrote it, and a lectionary that
//! opens any year, 40,000 and 41,000 included, instantly.

use crate::art::Artwork;
use crate::canon::{self, Testament};
use crate::html::{base64, esc};
use crate::render::{Ctx, thousands};
use computus::{MOVABLE, jdn_to_gregorian, pascha_jdn, pascha_julian, weekday};
use serde::Deserialize;
use std::fmt::Write as _;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

fn month_name(m: i64) -> &'static str {
    usize::try_from(m - 1)
        .ok()
        .and_then(|i| MONTHS.get(i))
        .copied()
        .unwrap_or("")
}

fn day_name(jdn: i64) -> &'static str {
    usize::try_from(weekday(jdn))
        .ok()
        .and_then(|i| DAYS.get(i))
        .copied()
        .unwrap_or("")
}

/// A year as printed: plain below 10,000 ("2026"), grouped above ("40,001").
#[must_use]
pub fn year_label(y: i64) -> String {
    if y < 10_000 {
        y.to_string()
    } else {
        thousands(usize::try_from(y).unwrap_or(0))
    }
}

/// "Sunday, 12 April 2026" for a Julian Day Number (civil, Gregorian).
#[must_use]
pub fn civil_label(jdn: i64) -> String {
    let d = jdn_to_gregorian(jdn);
    format!(
        "{}, {} {} {}",
        day_name(jdn),
        d.day,
        month_name(d.month),
        year_label(d.year)
    )
}

/// One book tile.
fn tile(ctx: &Ctx<'_>, bi: usize, href: &str, meta: &str, current: bool) -> String {
    let cur = if current {
        " aria-current=\"page\""
    } else {
        ""
    };
    format!(
        "<li><a class=\"book\" href=\"{}\"{cur}><span class=\"n\">{}</span><span class=\"m\">{}</span></a></li>",
        esc(href),
        esc(ctx.bible.book_name(bi).unwrap_or("")),
        esc(meta)
    )
}

/// The full book selector: every book of both Testaments, each one click from here.
#[must_use]
pub fn book_selector(ctx: &Ctx<'_>, current: Option<usize>, heading_level: u8) -> String {
    let mut h = String::from("<div class=\"shelf\">");
    for (label, t) in [
        ("Old Testament", Testament::Old),
        ("New Testament", Testament::New),
    ] {
        let _ = write!(
            h,
            "<h{heading_level}>{label}</h{heading_level}><ul class=\"books\">"
        );
        for (bi, info) in canon::BOOKS
            .iter()
            .enumerate()
            .filter(|(_, b)| b.testament == t)
        {
            let n = ctx.bible.chapter_count(bi);
            let meta = if n == 1 {
                "1 chapter".to_string()
            } else {
                format!("{n} chapters")
            };
            let href = ctx.cfg.href(&format!("{}/", info.slug));
            h.push_str(&tile(ctx, bi, &href, &meta, current == Some(bi)));
        }
        h.push_str("</ul>");
    }
    h.push_str("</div>");
    h
}

/// The selector folded into a disclosure, for reading pages.
#[must_use]
pub fn picker(ctx: &Ctx<'_>, current: Option<usize>, open: bool) -> String {
    format!(
        "<details class=\"picker\"{}><summary>All 66 books</summary>{}</details>",
        if open { " open" } else { "" },
        book_selector(ctx, current, 3)
    )
}

/// The art index's selector: only books that have art, each jumping to its section.
#[must_use]
pub fn art_selector(ctx: &Ctx<'_>, books: &[(usize, usize)], other: usize) -> String {
    let mut h = String::from("<div class=\"shelf\"><h2>Choose a book</h2><ul class=\"books\">");
    for &(bi, n) in books {
        let slug = canon::book(bi).map_or("", |b| b.slug);
        let meta = if n == 1 {
            "1 work".to_string()
        } else {
            format!("{n} works")
        };
        h.push_str(&tile(ctx, bi, &format!("#art-{slug}"), &meta, false));
    }
    if other > 0 {
        let _ = write!(
            h,
            "<li><a class=\"book\" href=\"#art-other\"><span class=\"n\">Beyond the 66</span><span class=\"m\">{other} works</span></a></li>"
        );
    }
    h.push_str("</ul></div>");
    h
}

/// Ezekiel 37 - the valley of dry bones - with Doré's plate when the manifest has it.
#[must_use]
pub fn ezekiel_37(ctx: &Ctx<'_>, plate: Option<&Artwork>) -> String {
    let verse = |v: usize| {
        format!(
            "<p><sup class=\"vn\">{v}</sup>{}</p>",
            ctx.verse_html(25, 37, v)
        )
    };
    let fig = plate.map_or_else(String::new, |w| ctx.feature_figure(w));
    format!(
        concat!(
            "<section class=\"feature\" aria-labelledby=\"ez37\">{fig}<div>",
            "<p class=\"eyebrow\">Featured chapter</p><h2 id=\"ez37\">Ezekiel 37 · The valley of dry bones</h2>",
            "<blockquote>{v4}{v5}{v12}<cite>Ezekiel 37:4-5, 12 · King James Version</cite></blockquote>",
            "<blockquote><p>{john}</p><cite>John 11:25</cite></blockquote>",
            "<p><a href=\"{href}\">Read Ezekiel 37</a></p></div></section>"
        ),
        fig = fig,
        v4 = verse(4),
        v5 = verse(5),
        v12 = verse(12),
        john = ctx.verse_html(42, 11, 25),
        href = esc(&ctx.verse_href_pub(25, 37, 1)),
    )
}

/// Psalm 115:1 with the site's motto and the verified Hebrew, Greek and Latin.
#[derive(Debug, Deserialize)]
pub struct Psalm115 {
    his_line: String,
    reference: String,
    hebrew: String,
    hebrew_source: String,
    greek: String,
    greek_source: String,
    latin: String,
    latin_source: String,
}

/// The Non Nobis motto block.
#[must_use]
pub fn non_nobis(ctx: &Ctx<'_>, p: &Psalm115, with_sources: bool) -> String {
    // The motto's words exactly as written; each language isolated for bidirectional
    // text. The separator is drawn as the Chi Rho: the only Christian symbols the site uses
    // are the Chi Rho and Alpha and Omega, so any cross glyph in the line is shown as the
    // Chi Rho. It is the shared emblem's letters (Junicode U+2627), never a text glyph left
    // to the visitor's fonts.
    let mut his = String::new();
    let sep = chi_rho::glyph("");
    for part in p
        .his_line
        .split(['☦', '☧'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let lang = if part.chars().any(|c| ('\u{0590}'..='\u{05FF}').contains(&c)) {
            "he\" dir=\"rtl"
        } else if part.chars().any(|c| {
            ('\u{0370}'..='\u{03FF}').contains(&c) || ('\u{1F00}'..='\u{1FFF}').contains(&c)
        }) {
            "grc"
        } else {
            "la"
        };
        let _ = write!(
            his,
            "<bdi lang=\"{lang}\">{}</bdi><span class=\"x\" aria-hidden=\"true\">{sep}</span>",
            esc(part)
        );
    }
    let src = |s: &str| {
        if with_sources {
            format!(" <span class=\"meta\">({})</span>", esc(s))
        } else {
            String::new()
        }
    };
    format!(
        concat!(
            "<section class=\"motto\" aria-label=\"Psalm 115:1\">",
            "<p class=\"his\">{his}</p>",
            "<p class=\"kjv\">\u{201c}{kjv}\u{201d} <a href=\"{href}\">Psalms 115:1</a></p>",
            "<div class=\"tri\">",
            "<p lang=\"he\" dir=\"rtl\"><span class=\"lab\" lang=\"en\" dir=\"ltr\">Hebrew</span>{he}{hs}</p>",
            "<p lang=\"grc\"><span class=\"lab\" lang=\"en\">Greek (Septuagint)</span>{gr}{gs}</p>",
            "<p lang=\"la\"><span class=\"lab\" lang=\"en\">Latin (Vulgate)</span>{la}{ls}</p>",
            "</div><p class=\"meta\">{reference}</p></section>"
        ),
        his = his,
        kjv = ctx.verse_html(18, 115, 1),
        href = esc(&ctx.verse_href_pub(18, 115, 1)),
        he = esc(&p.hebrew),
        hs = src(&p.hebrew_source),
        gr = esc(&p.greek),
        gs = src(&p.greek_source),
        la = esc(&p.latin),
        ls = src(&p.latin_source),
        reference = esc(&p.reference),
    )
}

/// The movable cycle of one year as an HTML table (computed, not looked up).
#[must_use]
pub fn cycle_table(year: i64) -> String {
    let p = pascha_jdn(year);
    let j = pascha_julian(year);
    let mut h = format!(
        "<p>Holy Pascha {yl}: <strong>{}</strong> (civil date, Gregorian calendar) · {} {} {yl} on the Julian calendar.</p><div class=\"tablewrap\"><table class=\"cycle\"><thead><tr><th scope=\"col\">Feast</th><th scope=\"col\">Civil date</th><th scope=\"col\">Days from Pascha</th></tr></thead><tbody>",
        civil_label(p),
        j.day,
        month_name(j.month),
        yl = year_label(year)
    );
    for m in &MOVABLE {
        let cls = if m.offset == 0 {
            " class=\"pascha\""
        } else {
            ""
        };
        let sign = if m.offset > 0 { "+" } else { "" };
        let _ = write!(
            h,
            "<tr{cls}><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{sign}{}</td></tr>",
            esc(m.name),
            civil_label(p + m.offset),
            m.offset
        );
    }
    h.push_str("</tbody></table></div>");
    h
}

/// Pascha for a run of years plus the far years, for readers without scripts.
#[must_use]
pub fn pascha_years(from: i64, to: i64, far: &[i64]) -> String {
    let mut h = String::from(
        "<div class=\"tablewrap\"><table class=\"cycle\"><thead><tr><th scope=\"col\">Year</th><th scope=\"col\">Holy Pascha (civil date)</th><th scope=\"col\">Julian calendar</th></tr></thead><tbody>",
    );
    for y in (from..=to).chain(far.iter().copied()) {
        let j = pascha_julian(y);
        let _ = write!(
            h,
            "<tr><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{} {}</td></tr>",
            year_label(y),
            civil_label(pascha_jdn(y)),
            j.day,
            month_name(j.month)
        );
    }
    h.push_str("</tbody></table></div>");
    h
}

/// The page script that runs the same Rust computus, compiled to WebAssembly, for any
/// year the reader types. Feast names come from the same table as the static pages.
#[must_use]
pub fn paschalion_script(wasm: &[u8]) -> String {
    let feasts = MOVABLE
        .iter()
        .map(|m| format!("[{},\"{}\"]", m.offset, m.name.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",");
    let months = MONTHS
        .iter()
        .map(|m| format!("\"{m}\""))
        .collect::<Vec<_>>()
        .join(",");
    let days = DAYS
        .iter()
        .map(|d| format!("\"{d}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        concat!(
            "(function(){{var W=\"{wasm}\",M=[{months}],D=[{days}],F=[{feasts}];",
            "function b(){{var s=atob(W),a=new Uint8Array(s.length);for(var i=0;i<s.length;i++)a[i]=s.charCodeAt(i);return a}}",
            "function n(v){{return v<10000?String(v):String(v).replace(/\\B(?=(\\d{{3}})+(?!\\d))/g,',')}}",
            "function f(x,j){{return D[(j+1)%7]+', '+x.civil_day(j)+' '+M[x.civil_month(j)-1]+' '+n(x.civil_year(j))}}",
            "document.addEventListener('DOMContentLoaded',function(){{var form=document.getElementById('pf');if(!form||!window.WebAssembly)return;",
            "WebAssembly.instantiate(b()).then(function(r){{var x=r.instance.exports,inp=document.getElementById('py'),out=document.getElementById('pout');form.hidden=false;",
            "function run(){{var y=parseInt(inp.value,10);if(!(y>=1&&y<=200000)){{out.textContent='Enter a year from 1 to 200,000.';return}}",
            "var p=x.pascha(y),h='<h3>'+n(y)+'</h3><p>Holy Pascha: <strong>'+f(x,p)+'</strong> (civil date, Gregorian calendar) \\u00b7 '+x.pascha_julian_day(y)+' '+M[x.pascha_julian_month(y)-1]+' '+n(y)+' on the Julian calendar.</p>';",
            "h+='<div class=\"tablewrap\"><table class=\"cycle\"><thead><tr><th scope=\"col\">Feast</th><th scope=\"col\">Civil date</th><th scope=\"col\">Days from Pascha</th></tr></thead><tbody>';",
            "F.forEach(function(e){{h+='<tr'+(e[0]===0?' class=\"pascha\"':'')+'><td>'+e[1]+'</td><td class=\"num\">'+f(x,p+e[0])+'</td><td class=\"num\">'+(e[0]>0?'+':'')+e[0]+'</td></tr>'}});",
            "out.innerHTML=h+'</tbody></table></div>'}}",
            "form.addEventListener('submit',function(e){{e.preventDefault();run()}});run()}})}})}})();"
        ),
        wasm = base64(wasm),
        months = months,
        days = days,
        feasts = feasts,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascha_2026_is_sunday_12_april() {
        assert_eq!(civil_label(pascha_jdn(2026)), "Sunday, 12 April 2026");
    }

    #[test]
    fn the_cycle_table_lists_pentecost_on_31_may_2026() {
        let t = cycle_table(2026);
        assert!(t.contains(
            "Pentecost: Feast of the Holy Trinity</td><td class=\"num\">Sunday, 31 May 2026"
        ));
        assert!(t.contains("Clean Monday)</td><td class=\"num\">Monday, 23 February 2026"));
    }
}

//! The contact sheet: every admitted plate on one page, with its evidence, so a
//! maintainer can look at each one before anything is published.
//!
//! > *"Prove all things; hold fast that which is good."* - 1 Thessalonians 5:21, KJV
//!
//! WHY a separate page: the build admits a work only on a recorded verdict, but the
//! decision to publish is a person's. The sheet shows each admitted image beside the facts
//! the verdict rests on (Commons licence, pixels, bytes, sha256, what was seen), and
//! lists every excluded file with its reason as text only. It is written outside the
//! built site, so it can never be deployed with it.

use crate::art::{Artwork, Exclusions, Manifest, Review, Verdict};
use crate::html::esc;
use crate::{Error, Result};
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

/// Counts reported after writing a contact sheet.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Works that passed the Commons licence gate and were fetched.
    pub fetched: usize,
    /// Files the licence gate refused.
    pub refused: usize,
    /// Works admitted by the content review (shown with their image).
    pub admitted: usize,
    /// Works excluded by the content review or the permanent exclusion list.
    pub excluded: usize,
    /// Of the counts above, the Doré plates: fetched, admitted, excluded.
    pub dore: (usize, usize, usize),
}

/// The relative path from `from_dir` to `to`, both absolute.
fn relative(from_dir: &Path, to: &Path) -> PathBuf {
    let from: Vec<Component<'_>> = from_dir.components().collect();
    let target: Vec<Component<'_>> = to.components().collect();
    let common = from.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in common..from.len() {
        out.push("..");
    }
    for part in target.iter().skip(common) {
        out.push(part.as_os_str());
    }
    out
}

/// Percent-encodes a relative path for an `href`/`src`, keeping `/`.
fn url_path(path: &Path) -> String {
    let mut out = String::new();
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// The served file for one width of a work (`art/img/<slug>-<w>.<hash>.jpg`).
fn served(dist: &Path, slug: &str, width: u32) -> Option<PathBuf> {
    let prefix = format!("{slug}-{width}.");
    std::fs::read_dir(dist.join("art/img"))
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
        })
}

fn bytes_label(n: u64) -> String {
    let kib = n / 1024;
    if kib >= 1024 {
        format!("{}.{} MB", kib / 1024, (kib % 1024) * 10 / 1024)
    } else {
        format!("{kib} KB")
    }
}

const PAGE_CSS: &str = r#":root{--bg:#fff;--ink:#111214;--muted:#5b6068;--line:#e6e7ea;--panel:#f7f7f8;--red:#b3121c;color-scheme:light}
:root[data-theme="dark"]{--bg:#0d0d0e;--ink:#ececef;--muted:#a3a8b0;--line:#26282d;--panel:#131416;--red:#ff5a64;color-scheme:dark}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif}
.wrap{max-width:96rem;margin:0 auto;padding:1.5rem 1.25rem 4rem}h1{font:600 2rem/1.2 "EB Garamond",Georgia,serif;margin:.4rem 0}
h2{font:600 1.4rem/1.2 "EB Garamond",Georgia,serif;margin:2.4rem 0 .8rem;border-top:1px solid var(--line);padding-top:1.2rem}
.lead{color:var(--muted);max-width:60rem}.counts{display:flex;flex-wrap:wrap;gap:.5rem 1.6rem;list-style:none;padding:0;margin:1rem 0}
.counts strong{display:block;font:600 1.6rem/1.1 Georgia,serif}.bar{display:flex;flex-wrap:wrap;gap:.6rem;align-items:center;margin:1rem 0}
button{min-height:40px;padding:0 .9rem;border:1px solid var(--ink);border-radius:.4rem;background:var(--ink);color:var(--bg);font:600 .85rem system-ui,sans-serif;cursor:pointer}
button.quiet{background:transparent;color:var(--ink)}button:hover{background:var(--red);border-color:var(--red);color:#fff}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(15rem,1fr));gap:1.2rem;list-style:none;padding:0;margin:0}
.card{border:1px solid var(--line);border-radius:.6rem;overflow:hidden;background:var(--panel);display:flex;flex-direction:column}
.card.no{outline:3px solid var(--red)}.card a.img{display:block;background:#fff;aspect-ratio:4/5}.card img{width:100%;height:100%;object-fit:contain;display:block}
.card .b{padding:.7rem .8rem .9rem;font-size:.82rem}.card .t{font:600 1.05rem/1.25 "EB Garamond",Georgia,serif;margin:.15rem 0 .3rem}
.n{font:600 .7rem system-ui,sans-serif;letter-spacing:.12em;color:var(--red)}.m{color:var(--muted);word-break:break-word}
.seen{margin:.4rem 0;padding:.35rem .5rem;border-left:3px solid var(--red);background:var(--bg)}
label.ok{display:flex;gap:.5rem;align-items:center;margin-top:.5rem;font-weight:600}label.ok input{width:18px;height:18px;accent-color:var(--red)}
table{border-collapse:collapse;width:100%;font-size:.85rem}th,td{text-align:left;vertical-align:top;padding:.45rem .5rem;border-bottom:1px solid var(--line)}
th{font:600 .7rem system-ui,sans-serif;letter-spacing:.12em;text-transform:uppercase;color:var(--muted)}.tw{overflow-x:auto}
textarea{width:100%;min-height:7rem;font:12px ui-monospace,Menlo,monospace;background:var(--panel);color:var(--ink);border:1px solid var(--line);border-radius:.4rem;padding:.6rem}
code{font:12px ui-monospace,Menlo,monospace}"#;

const PAGE_JS: &str = r"(function(){var d=document.documentElement,k='dore-sheet-theme',a='dore-sheet-no';
try{if(localStorage.getItem(k)==='dark')d.setAttribute('data-theme','dark')}catch(e){}
function no(){try{return JSON.parse(localStorage.getItem(a)||'[]')}catch(e){return[]}}
function save(v){try{localStorage.setItem(a,JSON.stringify(v))}catch(e){}}
function sync(){var v=no(),out=[];document.querySelectorAll('[data-file]').forEach(function(c){var off=v.indexOf(c.dataset.file)>=0;c.classList.toggle('no',off);var b=c.querySelector('input');if(b)b.checked=!off;if(off)out.push(c.dataset.file)});var t=document.getElementById('rejected');if(t)t.value=out.join('\n');var n=document.getElementById('nrej');if(n)n.textContent=out.length}
document.addEventListener('change',function(e){var c=e.target.closest('[data-file]');if(!c)return;var v=no().filter(function(f){return f!==c.dataset.file});if(!e.target.checked)v.push(c.dataset.file);save(v);sync()});
document.getElementById('theme').addEventListener('click',function(){var dark=d.getAttribute('data-theme')!=='dark';if(dark)d.setAttribute('data-theme','dark');else d.removeAttribute('data-theme');try{localStorage.setItem(k,dark?'dark':'light')}catch(e){}});
document.getElementById('allok').addEventListener('click',function(){save([]);sync()});sync()})();";

fn card(w: &Artwork, n: usize, review: &Review, dist: &Path, sheet_dir: &Path) -> Result<String> {
    let thumb = served(dist, &w.slug, 480)
        .or_else(|| served(dist, &w.slug, 720))
        .ok_or_else(|| {
            Error::Data(format!(
                "contact sheet: no served image for admitted work {}",
                w.commons_file
            ))
        })?;
    let thumb = Some(thumb);
    let large = served(dist, &w.slug, 1920)
        .or_else(|| served(dist, &w.slug, 1440))
        .or_else(|| thumb.clone());
    let rel = |p: &Option<PathBuf>| {
        p.as_ref()
            .map(|p| url_path(&relative(sheet_dir, p)))
            .unwrap_or_default()
    };
    let seen = review
        .rows()
        .find(|r| r.commons_file == w.commons_file)
        .map(|r| format!("{} ({}, {})", r.reason, r.reviewer, r.reviewed))
        .unwrap_or_default();
    let master = w.master.as_ref().map_or_else(String::new, |m| {
        format!(
            "Served from {}×{} px, {} · sha256 <code title=\"{}\">{}…</code>",
            m.width,
            m.height,
            bytes_label(m.bytes),
            esc(&m.sha256),
            esc(m.sha256.get(..16).unwrap_or(&m.sha256))
        )
    });
    Ok(format!(
        concat!(
            "<li class=\"card\" data-file=\"{file}\"><a class=\"img\" href=\"{large}\"><img src=\"{thumb}\" alt=\"{scene}\" loading=\"lazy\"></a>",
            "<div class=\"b\"><span class=\"n\">PLATE {n}</span><p class=\"t\">{scene}</p>",
            "<p class=\"m\">{passage} · {artist}, {date}</p>",
            "<p class=\"m\"><a href=\"{url}\">{file}</a><br>Licence (Commons API): {licence} (<code>{code}</code>)<br>Original {ow}×{oh} px, {ob}<br>{master}</p>",
            "<p class=\"seen\">Seen: {seen}</p>",
            "<label class=\"ok\"><input type=\"checkbox\" checked> Approve for the site</label></div></li>"
        ),
        file = esc(&w.commons_file),
        large = rel(&large),
        thumb = rel(&thumb),
        scene = esc(&w.scene),
        n = n,
        passage = esc(w
            .passage
            .as_deref()
            .or(w.other_ref.as_deref())
            .unwrap_or("no passage")),
        artist = esc(&w.artist),
        date = esc(&w.date),
        url = esc(&w.commons_url),
        licence = esc(&w.licence_short),
        code = esc(&w.licence_code),
        ow = w.original_size.0,
        oh = w.original_size.1,
        ob = bytes_label(w.original_bytes),
        master = master,
        seen = esc(&seen),
    ))
}

/// One table row for a work that is not published, with the reason it was left out.
fn excluded_row(w: &Artwork, review: &Review, legacy: &Exclusions) -> String {
    let row = review.rows().find(|r| r.commons_file == w.commons_file);
    let reason = row.map_or_else(
        || "Not reviewed, so never published".to_string(),
        |r| {
            if legacy.excludes(&w.commons_file) {
                format!(
                    "{} (also on the permanent 2026-10-12 exclusion list)",
                    r.reason
                )
            } else {
                r.reason.clone()
            }
        },
    );
    format!(
        "<tr><td>{}</td><td>{}</td><td><a href=\"{}\">{}</a></td><td>{}</td><td>{}</td></tr>",
        esc(&w.scene),
        esc(w
            .passage
            .as_deref()
            .or(w.other_ref.as_deref())
            .unwrap_or("")),
        esc(&w.commons_url),
        esc(&w.commons_file),
        esc(row.map_or("NOT REVIEWED", |r| r.rule.as_str())),
        esc(&reason),
    )
}

/// The whole contact-sheet document.
fn page(c: &Counts, cards: &str, excluded: &str, refused: &str) -> String {
    let mut h = String::new();
    let _ = write!(
        h,
        concat!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">",
            "<title>Doré plates: contact sheet</title><style>{css}</style></head><body><div class=\"wrap\">",
            "<p class=\"n\">THE SCRIPTURES · CONTENT REVIEW · NOT PUBLISHED</p><h1>Every admitted plate, for your approval</h1>",
            "<p class=\"lead\">Each image below was looked at one by one under the content rule (no nudity, no gore shown for shock; clothed people of any age are fine; when unsure, it is left out). ",
            "Untick any plate you do not want on the site; the list of unticked files collects at the bottom for you to send back. Nothing here is live.</p>",
            "<ul class=\"counts\"><li><strong>{fetched}</strong>fetched through the licence gate</li><li><strong>{refused}</strong>refused by the licence gate</li>",
            "<li><strong>{admitted}</strong>admitted</li><li><strong>{excluded}</strong>excluded</li>",
            "<li><strong>{df} / {da} / {dx}</strong>Doré fetched / admitted / excluded</li></ul>",
            "<div class=\"bar\"><button type=\"button\" id=\"theme\" class=\"quiet\">Dark / light</button><button type=\"button\" id=\"allok\" class=\"quiet\">Tick every plate again</button></div>",
            "<h2>Admitted ({admitted})</h2><ul class=\"grid\">{cards}</ul>",
            "<h2>Unticked by you (<span id=\"nrej\">0</span>)</h2><textarea id=\"rejected\" readonly aria-label=\"Commons files you unticked\"></textarea>",
            "<h2>Excluded ({excluded}), listed without images</h2><div class=\"tw\"><table><thead><tr><th>Scene</th><th>Passage</th><th>Commons file</th><th>Rule</th><th>What was seen</th></tr></thead><tbody>{excl}</tbody></table></div>",
            "<h2>Refused by the licence gate ({refused})</h2><div class=\"tw\"><table><thead><tr><th>Commons file</th><th>Reason</th></tr></thead><tbody>{refl}</tbody></table></div>",
            "</div><script>{js}</script></body></html>\n"
        ),
        css = PAGE_CSS,
        js = PAGE_JS,
        fetched = c.fetched,
        refused = c.refused,
        admitted = c.admitted,
        excluded = c.excluded,
        df = c.dore.0,
        da = c.dore.1,
        dx = c.dore.2,
        cards = cards,
        excl = excluded,
        refl = refused,
    );
    h
}

/// Writes the contact sheet for a data directory and the site built from it.
///
/// # Errors
/// Refuses an existing output file, a missing manifest, or a build without images.
pub fn write(data: &Path, dist: &Path, out: &Path) -> Result<Counts> {
    use std::io::Write as _;
    let manifest = Manifest::load(&data.join("art_manifest.json"))?;
    let review = Review::load(&data.join("content_review.tsv"))?;
    let legacy = Exclusions::load(&data.join("ag_child_removed.tsv"))?;
    let dist = dist
        .canonicalize()
        .map_err(|e| Error::io("contact sheet: built site", dist, e))?;
    let parent = out
        .parent()
        .ok_or_else(|| Error::Usage("contact sheet needs a file path".into()))?;
    std::fs::create_dir_all(parent).map_err(|e| Error::io("create dir", parent, e))?;
    let sheet_dir = parent
        .canonicalize()
        .map_err(|e| Error::io("contact sheet dir", parent, e))?;
    let mut c = Counts {
        fetched: manifest.works.len(),
        refused: manifest.refused.len(),
        ..Counts::default()
    };
    let is_dore = |w: &Artwork| w.artist_key.starts_with("dore");
    let mut cards = String::new();
    let mut excluded = String::new();
    for w in &manifest.works {
        let admitted = !legacy.excludes(&w.commons_file)
            && review.verdict(&w.commons_file) == Some(Verdict::Admit);
        if is_dore(w) {
            c.dore.0 += 1;
        }
        if admitted {
            c.admitted += 1;
            if is_dore(w) {
                c.dore.1 += 1;
            }
            cards.push_str(&card(w, c.admitted, &review, &dist, &sheet_dir)?);
        } else {
            c.excluded += 1;
            if is_dore(w) {
                c.dore.2 += 1;
            }
            excluded.push_str(&excluded_row(w, &review, &legacy));
        }
    }
    let mut refused = String::new();
    for (file, reason) in &manifest.refused {
        let _ = write!(
            refused,
            "<tr><td>{}</td><td>{}</td></tr>",
            esc(file),
            esc(reason)
        );
    }
    let h = page(&c, &cards, &excluded, &refused);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(|e| Error::io("new contact sheet", out, e))?;
    file.write_all(h.as_bytes())
        .map_err(|e| Error::io("write contact sheet", out, e))?;
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_climb_to_the_common_ancestor() {
        assert_eq!(
            relative(
                Path::new("/a/b/review"),
                Path::new("/a/b/site/art/img/x.jpg")
            ),
            PathBuf::from("../site/art/img/x.jpg")
        );
        assert_eq!(
            url_path(Path::new("../doré site/a b.jpg")),
            "../dor%C3%A9%20site/a%20b.jpg"
        );
    }

    #[test]
    fn sizes_read_in_kilobytes_and_megabytes() {
        assert_eq!(bytes_label(2048), "2 KB");
        assert_eq!(bytes_label(5_690_000), "5.4 MB");
    }
}

//! The red-letter edition, the prayer rule and the reader's private profile page.
//!
//! > *"Heaven and earth shall pass away: but my words shall not pass away."*
//! > - Mark 13:31, KJV
//!
//! Every Scripture quoted here is read from the on-disk King James text at build time,
//! so nothing is retyped: the Lord's Prayer is Matthew 6:9-13 exactly as the data holds
//! it, and every milestone verse is the red-letter span of its verse, verbatim.

use crate::canon;
use crate::html::{self, PageMeta, esc};
use crate::render::{Ctx, red_html, thousands};
use crate::xref;
use crate::{Error, Result};
use std::fmt::Write as _;

/// Verses whose words of Christ the profile page quotes at its milestone levels:
/// (level, book index, chapter, verse). Level 0 marks the rotation used for every
/// hundredth level past 99.
pub const MILESTONE_VERSES: [(u64, usize, usize, usize); 10] = [
    (10, 39, 11, 28),
    (50, 42, 15, 5),
    (92, 39, 5, 6),
    (99, 42, 11, 25),
    (0, 42, 14, 6),
    (0, 42, 8, 12),
    (0, 42, 6, 35),
    (0, 39, 6, 33),
    (0, 40, 13, 31),
    (0, 39, 28, 20),
];

/// The words of Christ in one verse, verbatim: its red-letter spans joined.
///
/// # Errors
/// Fails when the verse has no words of Christ (a milestone may only quote Him).
pub fn red_words(ctx: &Ctx<'_>, book: usize, chapter: usize, verse: usize) -> Result<String> {
    let text = ctx
        .bible
        .verse_text(book, chapter, verse)
        .ok_or_else(|| Error::Data(format!("no verse {book} {chapter}:{verse}")))?;
    let parts: Vec<&str> = ctx
        .red
        .spans(&xref::osis_key(book, chapter, verse))
        .iter()
        .filter_map(|&(s, e)| text.get(s..e))
        .collect();
    if parts.is_empty() {
        return Err(Error::Data(format!(
            "{} {chapter}:{verse} has no words of Christ",
            ctx.bible.book_name(book).unwrap_or("?")
        )));
    }
    Ok(parts.join(" "))
}

fn verse_ref(ctx: &Ctx<'_>, book: usize, chapter: usize, verse: usize) -> String {
    format!(
        "{} {chapter}:{verse}",
        ctx.bible.book_name(book).unwrap_or("")
    )
}

/// The verses of one book that carry the words of Christ, by chapter.
fn red_verses(ctx: &Ctx<'_>, book: usize) -> Vec<(usize, Vec<usize>)> {
    let mut out = Vec::new();
    for chapter in 1..=ctx.bible.chapter_count(book) {
        let verses: Vec<usize> = (1..=ctx.bible.verse_count(book, chapter))
            .filter(|v| !ctx.red.spans(&xref::osis_key(book, chapter, *v)).is_empty())
            .collect();
        if !verses.is_empty() {
            out.push((chapter, verses));
        }
    }
    out
}

/// Books that carry the words of Christ, with their verse counts, in canonical order.
#[must_use]
pub fn red_books(ctx: &Ctx<'_>) -> Vec<(usize, usize)> {
    (0..canon::BOOKS.len())
        .filter_map(|book| {
            let n: usize = red_verses(ctx, book).iter().map(|(_, v)| v.len()).sum();
            (n > 0).then_some((book, n))
        })
        .collect()
}

/// The red-letter index: every book in which JESUS CHRIST speaks.
#[must_use]
pub fn red_letter_index(ctx: &Ctx<'_>) -> String {
    let books = red_books(ctx);
    let total: usize = books.iter().map(|(_, n)| n).sum();
    let mut b = String::from("<div class=\"wrap\">");
    let _ = write!(
        b,
        "<nav class=\"crumbs\" aria-label=\"Breadcrumb\"><ol><li><a href=\"{}\">The Scriptures</a></li><li aria-current=\"page\">Red Letter</li></ol></nav><h1>The words of Christ</h1><p class=\"sub\">Every saying of <strong>JESUS CHRIST</strong> in the King James Version, book by book: {} verses, each linked to its place in the chapter. The words are marked from the red-letter King James text of eBible.org.</p>",
        esc(&ctx.cfg.href("")),
        thousands(total)
    );
    let _ = write!(
        b,
        "<blockquote><p>{}</p><cite><a href=\"{}\">Mark 13:31</a></cite></blockquote><ul class=\"rl-books\">",
        ctx.verse_html(40, 13, 31),
        esc(&ctx.verse_href_pub(40, 13, 31))
    );
    for (book, n) in &books {
        let slug = canon::book(*book).map_or("", |x| x.slug);
        let _ = write!(
            b,
            "<li><a href=\"{}\"><span>{}</span><span class=\"m\">{} verse{}</span></a></li>",
            esc(&ctx.cfg.href(&format!("red-letter/{slug}/"))),
            esc(ctx.bible.book_name(*book).unwrap_or("")),
            thousands(*n),
            if *n == 1 { "" } else { "s" }
        );
    }
    let _ = write!(
        b,
        "</ul><p class=\"meta\">Search only His words: <a href=\"{}?red_letter=1\">search the words of Christ</a>. In any chapter, choose \u{201c}Words of Christ only\u{201d} to read His words alone.</p></div>",
        esc(&ctx.cfg.href("search/"))
    );
    html::page(
        ctx.cfg,
        &PageMeta {
            title: "The Words of Christ in Red \u{b7} King James Version \u{b7} The Scriptures"
                .to_string(),
            description: format!(
                "Every saying of Jesus Christ in the King James Bible, book by book: {} red-letter verses, each linked to its chapter.",
                thousands(total)
            ),
            path: "red-letter/".to_string(),
            og_type: "website",
            section: "red-letter",
            ..PageMeta::default()
        },
        &b,
    )
}

/// One book's red-letter page: each verse with His words, the narrative kept plain.
#[must_use]
pub fn red_letter_book(ctx: &Ctx<'_>, book: usize) -> String {
    let name = ctx.bible.book_name(book).unwrap_or("").to_string();
    let slug = canon::book(book).map_or("", |x| x.slug);
    let chapters = red_verses(ctx, book);
    let total: usize = chapters.iter().map(|(_, v)| v.len()).sum();
    let mut b = String::from("<div class=\"wrap\">");
    let _ = write!(
        b,
        "<nav class=\"crumbs\" aria-label=\"Breadcrumb\"><ol><li><a href=\"{}\">The Scriptures</a></li><li><a href=\"{}\">Red Letter</a></li><li aria-current=\"page\">{}</li></ol></nav><h1>The words of Christ in {}</h1><p class=\"sub\">{} verse{} \u{b7} King James Version</p>",
        esc(&ctx.cfg.href("")),
        esc(&ctx.cfg.href("red-letter/")),
        esc(&name),
        esc(&name),
        thousands(total),
        if total == 1 { "" } else { "s" }
    );
    for (chapter, verses) in &chapters {
        let _ = write!(
            b,
            "<h2 class=\"chapter-head\"><a href=\"{}\">{} {chapter}</a></h2><ul class=\"sayings\">",
            esc(&ctx.cfg.href(&format!("{slug}/{chapter}/"))),
            esc(&name)
        );
        for v in verses {
            let text = ctx.bible.verse_text(book, *chapter, *v).unwrap_or_default();
            let _ = write!(
                b,
                "<li id=\"c{chapter}v{v}\"><span class=\"ref\"><a href=\"{}\">{} {chapter}:{v}</a></span><span class=\"text\">{}</span></li>",
                esc(&ctx.verse_href_pub(book, *chapter, *v)),
                esc(&name),
                red_html(&text, ctx.red.spans(&xref::osis_key(book, *chapter, *v)))
            );
        }
        b.push_str("</ul>");
    }
    let books = red_books(ctx);
    let at = books.iter().position(|(bk, _)| *bk == book).unwrap_or(0);
    let (prev, next) = crate::gallery::neighbours(&books, at);
    b.push_str("<nav class=\"pager\" aria-label=\"Pages\">");
    for (item, arrow_before) in [(prev, true), (next, false)] {
        if let Some((bk, _)) = item {
            let s = canon::book(*bk).map_or("", |x| x.slug);
            let label = ctx.bible.book_name(*bk).unwrap_or("");
            let _ = write!(
                b,
                "<a rel=\"{}\" href=\"{}\">{}{}{}</a>",
                if arrow_before { "prev" } else { "next" },
                esc(&ctx.cfg.href(&format!("red-letter/{s}/"))),
                if arrow_before { "\u{2190} " } else { "" },
                esc(label),
                if arrow_before { "" } else { " \u{2192}" }
            );
        }
        if arrow_before {
            let _ = write!(
                b,
                "<a href=\"{}\">All the books</a>",
                esc(&ctx.cfg.href("red-letter/"))
            );
        }
    }
    b.push_str("</nav></div>");
    html::page(
        ctx.cfg,
        &PageMeta {
            title: format!(
                "The Words of Christ in {name} \u{b7} Red Letter KJV \u{b7} The Scriptures"
            ),
            description: format!(
                "Every saying of Jesus Christ in {name}, King James Version: {} red-letter verses, each linked to its chapter.",
                thousands(total)
            ),
            path: format!("red-letter/{slug}/"),
            og_type: "website",
            section: "red-letter",
            ..PageMeta::default()
        },
        &b,
    )
}

fn passage(ctx: &Ctx<'_>, book: usize, chapter: usize, from: usize, to: usize) -> String {
    let mut p = String::from("<p>");
    for v in from..=to {
        let _ = write!(
            p,
            "<sup class=\"vn\">{v}</sup>{} ",
            ctx.verse_html(book, chapter, v)
        );
    }
    let label = if from == to {
        verse_ref(ctx, book, chapter, from)
    } else {
        format!(
            "{} {chapter}:{from}\u{2013}{to}",
            ctx.bible.book_name(book).unwrap_or("")
        )
    };
    let _ = write!(
        p,
        "</p><cite><a href=\"{}\">{label}</a></cite>",
        esc(&ctx.verse_href_pub(book, chapter, from))
    );
    p
}

/// The prayer rule.
///
/// Morning and evening prayers from the Psalms and the Gospels, the Lord's Prayer as
/// Matthew 6:9-13 gives it, the Jesus Prayer with a quiet counter, and the reader's own
/// named prayers (names only; no text is supplied by the site).
#[must_use]
pub fn prayer(ctx: &Ctx<'_>) -> String {
    let lords_prayer = passage(ctx, 39, 6, 9, 13);
    let mut b = String::from("<div class=\"wrap\" id=\"prayer\">");
    let _ = write!(
        b,
        "<nav class=\"crumbs\" aria-label=\"Breadcrumb\"><ol><li><a href=\"{}\">The Scriptures</a></li><li aria-current=\"page\">Prayer</li></ol></nav><h1>A rule of prayer</h1><p class=\"sub\">Morning and evening prayers in the words of Scripture, the Lord's Prayer, and the Jesus Prayer. Every text is the King James Version, except the Jesus Prayer, the Church's ancient prayer, given with the verses it grows from.</p>",
        esc(&ctx.cfg.href(""))
    );
    let _ = write!(
        b,
        "<section aria-labelledby=\"morning\"><h2 id=\"morning\">Morning prayers</h2><div class=\"prayer-text\">{}{}{}<h3>The Lord's Prayer</h3>{lords_prayer}</div><p><button type=\"button\" class=\"btn quiet\" data-pray=\"morning\" hidden>I have said the morning prayers</button></p></section>",
        passage(ctx, 18, 51, 15, 15),
        passage(ctx, 18, 5, 3, 3),
        passage(ctx, 18, 143, 8, 8)
    );
    let _ = write!(
        b,
        "<section aria-labelledby=\"evening\"><h2 id=\"evening\">Evening prayers</h2><div class=\"prayer-text\">{}{}<h3>The Song of Simeon</h3>{}<h3>The Lord's Prayer</h3>{lords_prayer}</div><p><button type=\"button\" class=\"btn quiet\" data-pray=\"evening\" hidden>I have said the evening prayers</button></p></section>",
        passage(ctx, 18, 141, 2, 2),
        passage(ctx, 18, 4, 8, 8),
        passage(ctx, 41, 2, 29, 32)
    );
    let _ = write!(
        b,
        "<section aria-labelledby=\"jesus-prayer\"><h2 id=\"jesus-prayer\">The Jesus Prayer</h2><div class=\"prayer-text\"><p>Lord Jesus Christ, Son of God, have mercy on me, a sinner.</p><p class=\"meta\">The prayer grows from these words of the Gospel:</p>{}{}</div>",
        passage(ctx, 41, 18, 38, 38),
        passage(ctx, 41, 18, 13, 13)
    );
    b.push_str("<div class=\"counter\" id=\"counter\" hidden><button type=\"button\" class=\"knot\" id=\"knot\" aria-describedby=\"count\">Once more</button><div><p class=\"count\" id=\"count\" aria-live=\"polite\">0</p><p class=\"meta\">Each press counts one prayer, like a knot on a prayer rope. Your count stays in this browser.</p><p class=\"form-row\"><button type=\"button\" class=\"btn quiet\" id=\"count-save\">Keep this count</button><button type=\"button\" class=\"link-button\" id=\"count-reset\">Start again</button></p></div></div></section>");
    b.push_str("<section aria-labelledby=\"own\" id=\"own-prayers\" hidden><h2 id=\"own\">Your own prayers</h2><p class=\"meta\">Name a prayer you keep (for your family, a person, a need). The name stays in this browser; no text is supplied here.</p><ul class=\"notes\" id=\"own-list\"></ul><form class=\"form-row\" id=\"own-add\"><label class=\"sr\" for=\"own-name\">Name of a prayer</label><input type=\"text\" id=\"own-name\" maxlength=\"40\" placeholder=\"For my family\" autocomplete=\"off\"><button type=\"submit\" class=\"btn quiet\">Add to my rule</button></form></section>");
    let _ = write!(
        b,
        "<p class=\"status\" id=\"prayer-status\" role=\"status\"></p><noscript><p class=\"meta\">The prayers above read the same without scripting. Keeping a count and a private record needs JavaScript, and stays in this browser.</p></noscript><p class=\"meta\">Keeping a private record of your prayer is optional and starts from <a href=\"{}\">your profile</a>.</p></div>",
        esc(&ctx.cfg.href("profile/"))
    );
    html::page(
        ctx.cfg,
        &PageMeta {
            title: "A Rule of Prayer: Morning, Evening and the Jesus Prayer \u{b7} The Scriptures"
                .to_string(),
            description: "Morning and evening prayers in the words of the King James Bible, the Lord's Prayer from Matthew 6:9-13, and the Jesus Prayer with a quiet counter.".to_string(),
            path: "prayer/".to_string(),
            og_type: "website",
            section: "prayer",
            ..PageMeta::default()
        },
        &b,
    )
}

/// The profile page: a private record of the reader's own walk, kept in their browser.
///
/// # Errors
/// Fails when a milestone verse carries no words of Christ.
pub fn profile(ctx: &Ctx<'_>) -> Result<String> {
    let mut words = String::from("<ol id=\"milestone-words\" hidden>");
    for (level, book, chapter, verse) in MILESTONE_VERSES {
        let _ = write!(
            words,
            "<li data-level=\"{level}\" data-ref=\"{}\" data-href=\"{}\">{}</li>",
            esc(&verse_ref(ctx, book, chapter, verse)),
            esc(&ctx.verse_href_pub(book, chapter, verse)),
            esc(&red_words(ctx, book, chapter, verse)?)
        );
    }
    words.push_str("</ol>");
    let mut books = String::from("<ol id=\"book-names\" hidden>");
    for info in &canon::BOOKS {
        let _ = write!(
            books,
            "<li data-slug=\"{}\" data-href=\"{}\">{}</li>",
            esc(info.slug),
            esc(&ctx.cfg.href(&format!("{}/", info.slug))),
            esc(info.name)
        );
    }
    books.push_str("</ol>");
    let mut b = String::from("<div class=\"wrap\" id=\"profile\">");
    let _ = write!(
        b,
        concat!(
            "<nav class=\"crumbs\" aria-label=\"Breadcrumb\"><ol><li><a href=\"{home}\">The Scriptures</a></li><li aria-current=\"page\">Profile</li></ol></nav>",
            "<h1>Your record</h1>",
            "<p class=\"sub\">A private record of your own reading, prayer, memory and study, if you would like one. It is for you alone: there is no ranking and nothing to compare.</p>",
            "<section class=\"panel\" aria-labelledby=\"privacy\"><h2 id=\"privacy\" class=\"flush\">Where your record is kept</h2>",
            "<p>Your progress stays in this browser (its own storage and one cookie for this site only) and in any file you download. It is never sent to us, and this site has no accounts. Download your progress to keep it safe or to move it to another device.</p></section>",
            "<div id=\"profile-start\" hidden><h2>Begin a record</h2><form class=\"form-row\" id=\"start-form\"><label class=\"sr\" for=\"start-name\">Your name</label><input type=\"text\" id=\"start-name\" maxlength=\"40\" placeholder=\"Your name\" autocomplete=\"off\" required><button type=\"submit\" class=\"btn\">Begin</button></form>",
            "<p class=\"meta\">Or <button type=\"button\" class=\"link-button\" data-load>load your progress from a file</button>.</p></div>",
            "<div id=\"profile-view\" hidden>",
            "<div class=\"profile-head\"><span class=\"initial\" id=\"p-initial\"></span><div><p class=\"eyebrow flush\">Profile</p><h2 class=\"flush\" id=\"p-name\"></h2></div>",
            "<svg class=\"ring\" viewBox=\"0 0 100 100\" role=\"img\" aria-labelledby=\"ring-label\"><title id=\"ring-label\">Total level</title><circle class=\"track\" cx=\"50\" cy=\"50\" r=\"45\"/><circle class=\"fill\" id=\"ring-fill\" cx=\"50\" cy=\"50\" r=\"45\" stroke-dasharray=\"0 282.743\"/><text x=\"50\" y=\"55\" text-anchor=\"middle\" id=\"ring-text\"></text></svg></div>",
            "<div class=\"totals\" id=\"p-totals\"></div>",
            "<h2>Skills</h2><ul class=\"skills\" id=\"p-skills\"></ul>",
            "<h2>The books</h2><p class=\"meta\" id=\"p-readings\"></p><ol class=\"bookmap\" id=\"p-books\"></ol>",
            "<h2>Kept for memory</h2><p class=\"meta\">Open any verse in a chapter and choose \u{201c}Keep for memory\u{201d}. Then say it back here.</p><div id=\"p-memory\"></div>",
            "<h2>Milestones</h2><div id=\"p-milestones\"><p class=\"meta\">None yet.</p></div>",
            "<h2>Recently</h2><ul class=\"notes\" id=\"p-notes\"></ul>",
            "<h2>Your progress file</h2><p class=\"form-row\"><button type=\"button\" class=\"btn quiet\" data-download>Download my progress</button><button type=\"button\" class=\"btn quiet\" data-load>Load my progress</button></p>",
            "<h2>Profiles on this device</h2><ul class=\"profiles\" id=\"p-profiles\"></ul>",
            "<form class=\"form-row\" id=\"add-form\"><label class=\"sr\" for=\"add-name\">Name for a new profile</label><input type=\"text\" id=\"add-name\" maxlength=\"40\" placeholder=\"Another reader's name\" autocomplete=\"off\"><button type=\"submit\" class=\"btn quiet\">Add a profile</button></form>",
            "<form class=\"form-row\" id=\"rename-form\"><label class=\"sr\" for=\"rename-name\">New name</label><input type=\"text\" id=\"rename-name\" maxlength=\"40\" placeholder=\"Rename this profile\" autocomplete=\"off\"><button type=\"submit\" class=\"btn quiet\">Rename</button></form>",
            "<h2>Settings</h2><label class=\"toggle\"><input type=\"checkbox\" id=\"set-record\"> Keep a private record</label><br><label class=\"toggle\"><input type=\"checkbox\" id=\"set-notes\"> Show a faint note at the page edge when something is recorded</label>",
            "<p class=\"form-row\"><button type=\"button\" class=\"link-button\" id=\"remove-profile\">Remove this profile</button><button type=\"button\" class=\"link-button\" id=\"clear-all\">Clear every record on this device</button></p>",
            "</div>",
            "<input type=\"file\" id=\"load-file\" accept=\"application/json,.json\" hidden>",
            "<p class=\"status\" id=\"p-status\" role=\"status\" aria-live=\"polite\"></p>",
            "<noscript><p class=\"meta\">A private record needs JavaScript; every chapter, prayer and plate reads the same without it.</p></noscript>",
            "<h2>How it is counted</h2><p class=\"meta\">A chapter counts once a day, after it has been on screen long enough to read and its last verse has been reached. Morning and evening prayers count once a day; the Jesus Prayer counts each prayer, up to three thousand a day. A verse said back from memory, a cross-reference followed, and a plate looked at for a while each count once a day. Levels 1 to 99 follow a long-established experience curve (level 99 is 13,034,431); past 99 every level is 6,000 more, without end. All of it is whole numbers.</p>",
            "{words}{books}</div>"
        ),
        home = esc(&ctx.cfg.href("")),
        words = words,
        books = books
    );
    Ok(html::page(
        ctx.cfg,
        &PageMeta {
            title: "Your Record \u{b7} The Scriptures".to_string(),
            description: "A private record of your own reading, prayer, memory and study, kept only in your browser and in your own file.".to_string(),
            path: "profile/".to_string(),
            og_type: "website",
            section: "profile",
            ..PageMeta::default()
        },
        &b,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn milestones_rotate_only_after_the_named_levels() {
        let named: Vec<u64> = MILESTONE_VERSES
            .iter()
            .map(|(l, ..)| *l)
            .filter(|l| *l > 0)
            .collect();
        assert_eq!(named, vec![10, 50, 92, 99]);
        assert!(MILESTONE_VERSES.iter().filter(|(l, ..)| *l == 0).count() >= 3);
    }
}

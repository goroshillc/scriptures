//! The Chi Rho emblem the Scriptures reader draws: in the header, the hero, the footer,
//! the favicon, the app icon and between the languages of the Psalm 115 motto.
//!
//! Why one crate: the emblem must be exact and the same everywhere it appears. Earlier
//! hand-made Chi Rhos (round-capped strokes, a geometric loop for the Rho) are replaced by
//! one master file, `chi-rho.svg`, so every use matches exactly.
//!
//! Why a font's glyph: sacred art is never generated or drawn by hand here. The letters are
//! U+2627 CHI RHO as Peter S. Baker drew it in Junicode 2.226 (SIL Open Font License 1.1),
//! extracted with fontTools by a generator that also sets the weight, the halo and the
//! optical centre by measured rules. See `PROVENANCE.md`.
#![forbid(unsafe_code)]
#![deny(warnings, clippy::pedantic, clippy::nursery)]

use std::sync::LazyLock;

/// The master emblem, byte for byte as the generator wrote it. Never edit it by hand.
pub const MASTER: &str = include_str!("../chi-rho.svg");

/// The emblem's canvas: 64 by 64 units with the halo centred at (32, 32). At 16 px one
/// unit is a quarter pixel, which is what lets the stem fall on whole pixels.
pub const VIEW_BOX: &str = "0 0 64 64";

/// The favicon's own stylesheet.
///
/// Black on white, and white on black when the visitor's system is dark. A site whose
/// Content-Security-Policy covers the icon must allow exactly this text by hash in
/// `style-src`; without it the icon still shows, black on white.
pub const FAVICON_STYLE: &str = ":root{color:#000}.ground{fill:#fff}@media (prefers-color-scheme:dark){:root{color:#fff}.ground{fill:#000}}";

/// The parts of the master that pages draw, as the exact attribute text of the master.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emblem {
    /// The `viewBox` that frames the letters alone, for the glyph beside text.
    pub glyph_view_box: &'static str,
    /// The halo's radius.
    pub halo_radius: &'static str,
    /// The halo's stroke width: the letters' mean stroke width (2 x area / perimeter).
    pub halo_stroke: &'static str,
    /// The letters' outline in canvas units, absolute commands only.
    pub path: &'static str,
}

/// The first `<tag .../>` element of the master.
fn element(tag: &str) -> Option<&'static str> {
    let open = format!("<{tag} ");
    let start = MASTER.find(&open)?;
    let rest = MASTER.get(start..)?;
    let end = rest.find("/>")?;
    rest.get(..end + 2)
}

/// One attribute's value inside an element (the leading space keeps `d` from matching `id`).
fn attr(element: &'static str, name: &str) -> Option<&'static str> {
    let key = format!(" {name}=\"");
    let start = element.find(&key)? + key.len();
    let rest = element.get(start..)?;
    let end = rest.find('"')?;
    rest.get(..end)
}

fn is_number(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn is_path_data(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            c.is_ascii_digit()
                || matches!(c, '.' | '-' | ' ' | 'M' | 'L' | 'H' | 'V' | 'Q' | 'C' | 'Z')
        })
}

/// Reads the master. `None` only if the file was edited out of the generator's shape; the
/// tests below make that a failed build, never a silent blank emblem.
fn parse() -> Option<Emblem> {
    let root = MASTER.get(..MASTER.find('>')?)?;
    let view = element("view")?;
    let halo = element("circle")?;
    let letters = element("path")?;
    let emblem = Emblem {
        glyph_view_box: attr(view, "viewBox")?,
        halo_radius: attr(halo, "r")?,
        halo_stroke: attr(halo, "stroke-width")?,
        path: attr(letters, "d")?,
    };
    let canvas_ok = root.contains(&format!(" viewBox=\"{VIEW_BOX}\""))
        && attr(halo, "cx") == Some("32")
        && attr(halo, "cy") == Some("32");
    let numbers_ok = is_number(emblem.halo_radius)
        && is_number(emblem.halo_stroke)
        && emblem
            .glyph_view_box
            .split(' ')
            .all(|v| is_number(v.trim_start_matches('-')));
    (canvas_ok && numbers_ok && is_path_data(emblem.path)).then_some(emblem)
}

static EMBLEM: LazyLock<Option<Emblem>> = LazyLock::new(parse);

/// The parsed master.
#[must_use]
pub fn emblem() -> Option<Emblem> {
    *EMBLEM
}

/// A `class` attribute keeping only letters, digits, spaces, hyphens and underscores, so a
/// caller's class can never break out of the markup.
fn class_attr(class: &str) -> String {
    let clean: String = class
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        String::new()
    } else {
        format!(" class=\"{clean}\"")
    }
}

impl Emblem {
    fn halo(&self) -> String {
        format!(
            "<circle cx=\"32\" cy=\"32\" r=\"{}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"{}\"/>",
            self.halo_radius, self.halo_stroke
        )
    }

    fn letters(&self) -> String {
        format!("<path fill=\"currentColor\" d=\"{}\"/>", self.path)
    }

    /// The emblem for an HTML page: decorative (`aria-hidden`), drawn in the text colour so
    /// it is black on white and white on black with the page. No `id`s, so it can appear
    /// any number of times on one page.
    #[must_use]
    pub fn inline(&self, class: &str) -> String {
        format!(
            "<svg{} viewBox=\"{VIEW_BOX}\" aria-hidden=\"true\" focusable=\"false\">{}{}</svg>",
            class_attr(class),
            self.halo(),
            self.letters()
        )
    }

    /// The letters alone, framed tightly, for use beside text (a separator in a line).
    #[must_use]
    pub fn glyph(&self, class: &str) -> String {
        format!(
            "<svg{} viewBox=\"{}\" aria-hidden=\"true\" focusable=\"false\">{}</svg>",
            class_attr(class),
            self.glyph_view_box,
            self.letters()
        )
    }

    /// The standalone favicon: the emblem on a round ground of the page colour, black on
    /// white, and white on black when the visitor's system is dark ([`FAVICON_STYLE`]). The
    /// ground keeps the halo visible on any tab bar.
    #[must_use]
    pub fn favicon(&self) -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{VIEW_BOX}\" color=\"#000\"><style>{FAVICON_STYLE}</style><circle class=\"ground\" cx=\"32\" cy=\"32\" r=\"32\" fill=\"#fff\"/>{}{}</svg>\n",
            self.halo(),
            self.letters()
        )
    }
}

/// [`Emblem::inline`] of the master.
#[must_use]
pub fn inline(class: &str) -> String {
    emblem().map(|e| e.inline(class)).unwrap_or_default()
}

/// [`Emblem::glyph`] of the master.
#[must_use]
pub fn glyph(class: &str) -> String {
    emblem().map(|e| e.glyph(class)).unwrap_or_default()
}

/// [`Emblem::favicon`] of the master.
#[must_use]
pub fn favicon() -> String {
    emblem().map(|e| e.favicon()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every (x, y) point of the outline, or `None` if a number does not parse. The
    /// generator writes absolute M, L, H, V, Q, C and Z only, so H carries an x, V a y, and
    /// everything else pairs.
    fn points(d: &str) -> Option<Vec<(f64, f64)>> {
        let mut out = Vec::new();
        let (mut x, mut y) = (0.0, 0.0);
        let mut command = 'M';
        let mut numbers: Vec<f64> = Vec::new();
        let mut token = String::new();
        // A NUL ends the data; numbers are grouped per command letter, never per space.
        for c in d.chars().chain(std::iter::once('\0')) {
            if c.is_ascii_digit() || c == '.' || (c == '-' && token.is_empty()) {
                token.push(c);
                continue;
            }
            if !token.is_empty() {
                numbers.push(token.parse().ok()?);
                token.clear();
            }
            if c == '-' {
                token.push(c);
                continue;
            }
            if c.is_ascii_alphabetic() || c == '\0' {
                let values = std::mem::take(&mut numbers);
                match command {
                    'H' => values.iter().for_each(|&v| {
                        x = v;
                        out.push((x, y));
                    }),
                    'V' => values.iter().for_each(|&v| {
                        y = v;
                        out.push((x, y));
                    }),
                    _ => values.as_chunks::<2>().0.iter().for_each(|&pair| {
                        (x, y) = pair.into();
                        out.push((x, y));
                    }),
                }
                command = c;
            }
        }
        Some(out)
    }

    fn num(s: &str) -> Option<f64> {
        s.parse().ok()
    }

    #[test]
    fn the_master_parses_into_halo_letters_and_glyph_frame() {
        let e = emblem();
        assert!(e.is_some(), "the master no longer parses");
        let Some(e) = e else { return };
        assert!(e.path.starts_with('M') && e.path.ends_with('Z'));
        assert_eq!(e.halo_radius, "30.54");
        assert_eq!(e.halo_stroke, "2.92");
        assert_eq!(e.glyph_view_box, "16 10 33 44");
    }

    #[test]
    fn the_halo_touches_the_canvas_edge_and_no_further() {
        let Some(e) = emblem() else { return };
        let outer = num(e.halo_radius)
            .zip(num(e.halo_stroke))
            .map(|(r, w)| r + w / 2.0);
        assert!(
            outer.is_some_and(|o| (o - 32.0).abs() < 0.011),
            "outer edge {outer:?}"
        );
    }

    #[test]
    fn every_point_of_the_letters_is_inside_the_halo_and_the_glyph_frame() {
        let Some(e) = emblem() else { return };
        let inner = num(e.halo_radius)
            .zip(num(e.halo_stroke))
            .map(|(r, w)| r - w / 2.0);
        let frame: Vec<f64> = e.glyph_view_box.split(' ').filter_map(num).collect();
        let pts = points(e.path);
        assert!(inner.is_some() && pts.is_some());
        let (Some(inner), Some(pts), [fx, fy, fw, fh]) = (inner, pts, frame.as_slice()) else {
            return;
        };
        assert!(pts.len() > 100, "only {} points", pts.len());
        for (x, y) in pts {
            assert!(
                (x - 32.0).hypot(y - 32.0) < inner,
                "({x}, {y}) touches the halo"
            );
            assert!(x >= *fx && x <= fx + fw, "x {x} outside the frame");
            assert!(y >= *fy && y <= fy + fh, "y {y} outside the frame");
        }
    }

    #[test]
    fn the_stem_is_one_whole_pixel_at_sixteen_pixels() {
        // The generator snaps the stem to x = 28..32 (pixel column 7 at 16 px); the outline
        // carries those two vertical edges at exactly 28 and 32.
        let pts = emblem().and_then(|e| points(e.path)).unwrap_or_default();
        assert!(pts.iter().any(|&(x, _)| (x - 28.0).abs() < 1e-9));
        assert!(pts.iter().any(|&(x, _)| (x - 32.0).abs() < 1e-9));
    }

    #[test]
    fn inline_markup_is_decorative_reusable_and_follows_the_text_colour() {
        let svg = inline("halo");
        assert!(svg.starts_with(
            "<svg class=\"halo\" viewBox=\"0 0 64 64\" aria-hidden=\"true\" focusable=\"false\">"
        ));
        assert!(!svg.contains(" id="), "ids would repeat on a page");
        assert!(svg.contains("stroke=\"currentColor\"") && svg.contains("fill=\"currentColor\""));
        assert!(!svg.contains('#'), "no fixed colour inline");
        assert_eq!(inline("").find(" class="), None);
    }

    #[test]
    fn a_hostile_class_cannot_break_out_of_the_markup() {
        let svg = inline("a\"><script>alert(1)</script>");
        assert!(!svg.contains("<script"));
        assert!(svg.starts_with("<svg class=\"ascriptalert1script\""));
    }

    #[test]
    fn the_glyph_alone_has_no_halo_and_its_own_frame() {
        let svg = glyph("x");
        assert!(svg.contains("viewBox=\"16 10 33 44\""));
        assert!(!svg.contains("<circle"));
    }

    #[test]
    fn the_favicon_is_black_on_white_and_white_on_black_in_the_dark() {
        let svg = favicon();
        assert!(svg.starts_with(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\" color=\"#000\">"
        ));
        assert!(svg.contains(&format!("<style>{FAVICON_STYLE}</style>")));
        assert!(svg.contains("class=\"ground\" cx=\"32\" cy=\"32\" r=\"32\" fill=\"#fff\""));
        assert!(
            FAVICON_STYLE.ends_with(
                "@media (prefers-color-scheme:dark){:root{color:#fff}.ground{fill:#000}}"
            )
        );
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn the_master_names_its_font_and_licence_and_carries_no_cross() {
        assert!(
            MASTER.contains(
                "U+2627, from Junicode 2.226 by Peter S. Baker (SIL Open Font License 1.1)"
            )
        );
        for symbol in [
            '\u{2720}', '\u{2629}', '\u{271d}', '\u{2626}', '\u{2020}', '\u{2627}',
        ] {
            assert!(!MASTER.contains(symbol), "{symbol} as text");
        }
    }
}

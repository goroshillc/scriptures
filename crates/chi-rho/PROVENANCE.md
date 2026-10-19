# Provenance

## Why

The emblem must be exact, refined and the same everywhere it appears. Before this crate, hand-made
Chi Rhos were drawn with round-capped strokes:
- `scriptures/src/html.rs` `emblem()` (48-unit grid, every page of the reader) and `ICON_SVG` (the favicon).
- The reader's Psalm 115 motto also printed `☧` as text, so each visitor's system font drew it.

All of them now come from the one master file, `chi-rho.svg`.

## Where the letters come from

| Item | Value |
|---|---|
| Glyph | U+2627 CHI RHO, glyph `uni2627` |
| Typeface | Junicode 2.226, Roman variable font, by Peter S. Baker |
| Release | https://github.com/psb1558/Junicode-font/releases/tag/v2.226 (published 2026-06-20) |
| Archive | `Junicode_2.226.zip`, sha256 `dae8ebc04a8592445c1878db8451838dd2f49d7c9912b3d4f3cab84c1516c5fe` |
| Font file | `Junicode/VAR/JunicodeVF-Roman.ttf`, 2,987,428 bytes, sha256 `f95e34a6b4fe2180c2366563bf39d2ed68abc4722aaa60cbc3fa9ef8aae58600` (checked by the generator, which refuses any other file) |
| Name table | version "Version 2.226"; copyright "Copyright (c) 1998-2026 by Peter S. Baker"; licence "Licensed under the SIL Open Font License, version 1.1" |
| Licence | SIL Open Font License 1.1, no Reserved Font Name; the release's `OFL.txt` is kept verbatim as `OFL-Junicode.txt` |

Use of the outline: the OFL FAQ (https://openfontlicense.org/ofl-faq/, question 1.1) says fonts under the OFL may be used "to create logos or other graphics or even to manufacture objects based on their outlines", and that "creating any kind of graphic using a font under the OFL does not make the resulting artwork subject to the OFL". The emblem is such a graphic; the font itself is not redistributed. The copyright line and licence are kept here and named on the reader's Sources page all the same.

## How the emblem is made

A generator (Python, fontTools 4.63.0 and skia-pathops 0.9.2, pinned in `tools/requirements.txt`) reads the glyph from the font at a point on its own weight axis, removes overlapping contours with skia-pathops (a union; no point is moved), and writes `chi-rho.svg`. Its rules, each measured from the outline:

- Canvas 64 x 64; one circle centred at (32, 32) whose outer edge touches the canvas edge.
- Weight: the font's `wght` axis solved so the stem is exactly 4 units, 1 px at 16 px (result: wght 426.33, between Junicode's Regular and Medium).
- Halo weight: the letters' mean stroke width, 2 x area / perimeter (result: 2.92 units), so the circle and the letters carry the same weight.
- Size: the farthest point of the letters from their reference centre is 80 % of the halo's inner radius, so the clearance to the circle is even all round (7.76 units above and below).
- Optical centre: horizontally the area centroid (the Rho's bowl carries weight right of the stem), vertically the bounding box's centre.
- Pixel grid: the stem's left edge snapped to the nearest multiple of 4 units (x = 28 to 32), a shift of 0.79 units, so the stem is one whole black pixel column at 16 px and whole columns at 32, 64 and 128 px.

The letters are never edited by hand. The generator script is not yet published in this
repository; until it is, `chi-rho.svg` is the master, and a change to the emblem starts with an
issue rather than an edit.

## How the candidates were compared

Each candidate was rendered by Chromium at 16, 32, 128 and 512 px in light and dark, at device pixel ratio 1 so the small sizes show the real pixels:

| Candidate | Source | Licence | Verdict |
|---|---|---|---|
| Noto Sans Symbols 2.003, ☧ (wght 300, 400, 500) | google/fonts commit 8b0a1d0f, `ofl/notosanssymbols` | OFL 1.1 | Clean, but a sans road-sign form beside the reader's Garamond; the bowl dominates the circle |
| Cardo 1.0451, ☧ | a local copy of the release | OFL 1.1 | Small Chi low on the stem; looks off-centre in a circle |
| STIX Two Math 2.13, ☧ | macOS system copy | OFL 1.1 | The Chi reads as an asterisk |
| EB Garamond 1.003, Χ over Ρ as a monogram | a local copy of the release | OFL 1.1 | Same-size letters merge into an "R/X"; rejected |
| Cinzel 2.000, X over P as a monogram (Cinzel has no Greek) | a local copy of the release | OFL 1.1 | Same; rejected |
| Libertinus 7.051, ☧ | https://github.com/alerque/libertinus/releases/tag/v7.051 | OFL 1.1 | Classical and close; lighter, Chi sits lower |
| **Junicode 2.226, ☧** | as above | OFL 1.1 | **Chosen**: classical bracketed serifs matching the reader's EB Garamond, Chi and Rho in balance, monumental within the halo |

The interruptions where the Chi's thin diagonal meets the stem are Junicode's own drawing, confirmed by rendering the font's text glyph in Chromium beside the extracted outline.

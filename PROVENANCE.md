# Provenance

Where every input came from, what was done to it, and how each step is checked.

## The text and the apparatus

| Input | Source | Licence | Check |
| --- | --- | --- | --- |
| `crates/scriptures/data/kjv.json` | The King James Version as used by the reader's original TypeScript engine | Public domain (Crown rights in the United Kingdom) | sha256 `34696cd96f71b6bc946717609c4663780924eca4cea7537615603ec6e4c56fdd`; `tests/parity.rs` |
| `crates/scriptures/data/xrefs.json` | Cross-references from the Treasury of Scripture Knowledge via [OpenBible.info](https://www.openbible.info/labs/cross-references/) | CC BY 4.0 (OpenBible.info); TSK public domain | sha256 `bcab99ad643992c2b764dfb2ceb6e4ac1732456de5e7a969b93d5023d03cd2d1`; `tests/parity.rs` |
| `crates/scriptures/data/redletter.json` | Derived by `fetch-redletter` from [eBible.org's KJV USFM](https://ebible.org/Scriptures/eng-kjv2006_usfm.zip) (`\wj` markup), aligned word by word onto the reader's text | Public domain (eBible.org) | the archive's sha256 is recorded inside the file |
| `crates/scriptures/data/lectionary.json` | Written by `fetch-lectionary` from [orthocal.info](https://orthocal.info/api/gregorian/); only dates, titles, names and references kept | orthocal software MIT, (c) 2022 Brian Glass | - |
| `crates/scriptures/data/psalm115.json` | Psalm 115:1 in Hebrew (Westminster Leningrad Codex), Greek (Swete, 1896) and Latin (Clementine Vulgate, 1592), with the site's motto | each source named in the file | - |

**The port.** `bible.rs` and `xref.rs` port the reader's original TypeScript engine
(`lib/scripture.ts`) line for line. `tests/parity.rs` holds the Rust to that engine's own output,
frozen in `tests/fixtures/engine_parity.json`: 336 sampled verses field by field, and all 31,102
verses by sha256. `tests/inscriptions.rs` checks every verse quoted in the source code against
`kjv.json` word for word.

## The art

1. **Plan.** `data/art_plan.tsv` names each Wikimedia Commons file and the passage it illustrates.
   Licences are not written there.
2. **Licence gate.** `fetch-art` reads each file's licence from the Commons API. Only `pd` (public
   domain) and `cc0` pass; anything else is refused, whatever its caption says. The result, with
   each file's Commons sha1, pixels and bytes, is `data/art_manifest.json`;
   [SOURCES.md](SOURCES.md) lists every work fetched and its licence verbatim.
3. **Content review.** Every image was looked at one by one, page scans cropped toward the plate
   and details zoomed where needed, and the verdict written to `data/content_review.tsv`: no
   nudity, no gore shown for shock, fully clothed people of any age are fine, and when unsure it
   is left out. `data/ag_child_removed.tsv` lists works removed earlier for showing unclothed
   children. The build refuses any work without a verdict and publishes only `ADMIT`: 217 works
   are admitted, 43 are not. The first review was made by an AI agent working under these rules;
   a maintainer approves each plate on the contact sheet (`goroshi-scriptures contact-sheet`),
   which shows every admitted image beside the facts its verdict rests on.
4. **Originals.** `fetch-originals` downloads each admitted file's original upload and refuses any
   whose sha1 differs from the sha1 Commons recorded for the reviewed file.
5. **Derivatives.** `derive-crisp` resizes (Lanczos3) and re-encodes only: AVIF (`avifenc`, q68),
   WebP (`cwebp`, q86, sharp YUV) and JPEG (`cjpeg` from libjpeg-turbo, q86, progressive) at
   480 to 2,560 pixels plus the full scan up to 6,000 pixels. Nothing is cropped, recoloured,
   sharpened or redrawn; an embedded colour profile is carried into every derivative. Every JPEG
   served carries XMP and IPTC metadata: the credit, the work's public-domain status and its
   Commons source. The encoder versions are recorded in `crisp.json`.
6. **Editions.** The website serves every format and width. The desktop app ships JPEG only at
   480 and 960 pixels plus one zoom image of at most 1,920 pixels (about 0.5 GB); its plates are
   the desktop art pack (`scripts/art-pack.sh`), pinned by sha256 in `data/art-pack.txt`.
7. **Wallpapers.** `goroshi-scriptures wallpapers` takes each verified original, resizes it
   (never enlarging), centres it on white at 1920 x 1080, 2560 x 1440, 2560 x 1600 and
   3840 x 2160, and writes `CREDITS.txt` beside them. A plate too small to fill half the screen is
   left out of that size rather than enlarged.

## Typography and the emblem

- **EB Garamond** (SIL Open Font License 1.1, no Reserved Font Name), subset to Latin, Greek and
  punctuation as WOFF with fontTools; licence text in `data/fonts/OFL.txt`.
- **The emblem** is the Chi Rho (U+2627) of **Junicode** by Peter S. Baker (SIL Open Font License
  1.1) within one circle, in `crates/chi-rho`; its provenance and licence are there. The only
  Christian symbols in the design are the Chi Rho and Alpha and Omega.

## What is not Rust, and why

| Piece | Language | Why |
| --- | --- | --- |
| `crates/scriptures/src/reader.js`, `discovery_ui.js` | JavaScript | Browser glue only: keys, clipboard, the zoom view, storage, and the DOM side of search. Every rule and number (search, ranking, the reading record) is Rust compiled to WebAssembly (`scripture-search`). Formatted and linted with Deno. |
| `crates/scriptures/src/reader.css` | CSS | The one stylesheet, inlined into each page and pinned by hash in the Content-Security-Policy. |
| `html.rs` `JS` constant (under 1 KB) | JavaScript | Sets the theme switch and marks the visitor's local date on the lectionary; every page reads with it off. Pinned by hash. |
| `wasm_check.rs` `PROBE` constant | JavaScript (Node) | Loads the generated WebAssembly binding so `check-search-wasm` can compare it with native Rust. Not shipped. |

## The app

`crates/scriptures-app` is Tauri 2 around the system webview. It answers its own `scriptures:`
scheme through `scriptures-host`'s router (the same path rules and Content-Security-Policy as the
website) from the desktop edition installed beside it. It opens no port and makes no network
request; links to other sites open in the system browser.

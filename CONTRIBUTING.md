# Contributing

Thank you for wanting to help. This reader is given freely to the glory of God, and Christians
of every nation and tradition, and anyone else of good will, are welcome to work on it.

> *"Go ye therefore, and teach all nations, baptizing them in the name of the Father, and of the
> Son, and of the Holy Ghost:"* - Matthew 28:19, KJV

## Ways to help

- **Report an error**: a verse shown wrongly, a cross-reference that resolves to the wrong place,
  a word of Christ not in red (or one that should not be), a broken link. Use the
  *Text or reference error* issue form and name the passage.
- **Propose a plate**: a public-domain master work that belongs at a passage (see the art rules
  below). Use the *Propose a plate* form.
- **Packaging**: help the app install cleanly on your system (Flatpak, winget, Homebrew, the AUR).
- **Accessibility and reading comfort**: screen readers, keyboard use, large type, contrast.
- **Code**: anything in the issues, or an improvement you can show with a test.

Discuss larger changes in an issue first, so no one's work is wasted.

## The rules

### 1. The King James text is never changed

The text is the King James Version exactly as in `crates/scriptures/data/kjv.json`. It is never
paraphrased, modernized, shortened or "corrected" by hand. Every verse quoted in the source code
is checked word for word against that file by `crates/scriptures/tests/inscriptions.rs`, and the
reader's text is held to the original engine by `tests/parity.rs` (all 31,102 verses by sha256).

### 2. Art: public-domain master works only, never generated

- Only works by recognised masters whose copyright has expired, taken from Wikimedia Commons.
  The licence is read from the Commons API for each file (`pd` or `cc0` only); a work without
  one is refused by `fetch-art`, whatever its caption says.
- **No generated, AI-made, redrawn, traced or "restored" images, ever.** Pixels are only resized
  and re-encoded; nothing is cropped, recoloured or sharpened. The sha1 of every downloaded
  original must equal the sha1 Commons recorded for the reviewed file.
- **Every image is looked at, one by one, before it is admitted**, and the verdict is written as a
  row of `crates/scriptures/data/content_review.tsv` (file, verdict, rule, what was seen, date,
  reviewer). The build refuses any work without a row and publishes only `ADMIT`.
  - No nudity: bare breasts, buttocks, genitals or an unclothed child are refused. A loincloth,
    or a cloth from the waist as on the cross, counts as covered.
  - No gore shown for shock.
  - Fully clothed people of any age are fine.
  - **When unsure, leave it out.** Children read this.
- A title or caption check is never enough. A maintainer looks at every proposed image before it
  is merged.

### 3. The symbols

The only Christian symbols in the design are the Chi Rho and Alpha and Omega. The emblem is the
Chi Rho of the Junicode typeface within one circle (`crates/chi-rho`); it is not redrawn.

### 4. Nothing leaves the reader's computer

No analytics, trackers, accounts, fonts or scripts from other sites, and no network request at
run time. The reading record stays in the reader's own browser storage or the app's, and in files
the reader chooses to download. Links to other sites open in the system browser.

### 5. Code quality

- Rust, formatted with `cargo fmt`, and `cargo clippy --workspace --all-targets -- -D warnings`
  clean with the crates' lints (pedantic and nursery; no `unsafe`; no `unwrap`, `expect`, `panic`,
  `as` casts or unchecked indexing outside tests).
- Tests for behaviour, named as sentences (`a_download_never_overwrites_an_existing_file`).
- Doc comments say *why*, not only what.
- JavaScript is browser glue only (DOM, storage, keys); every rule is Rust compiled to WebAssembly.
  It is formatted and linted with Deno (`deno fmt`, `deno lint` in `crates/scriptures`).
- Dependencies are pinned to exact versions, and a new one needs a reason in the pull request.
- `scripts/check.sh` runs everything CI runs.

### 6. Pull requests

- One change per pull request, with a plain description of what and why.
- Show the checks you ran; for a visible change, add a screenshot.
- By contributing you agree that your contribution is licensed under MIT or Apache-2.0, at the
  user's option, like the rest of the code.

Please follow the [Code of Conduct](CODE_OF_CONDUCT.md).

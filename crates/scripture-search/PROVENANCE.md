# scripture-search

Search, references and the private reading record of The Scriptures, in Rust. The same code runs
natively (the builder's tests and `check-search-wasm`) and in the browser as WebAssembly, so the
results cannot drift between the two.

- Matching, ranking, facets, highlights and pagination over the King James text and the admitted
  art; references typed straight in (`John 3:16`, `Ps 23`); King James spellings found from
  modern ones.
- The reading record (`progress.rs`): crediting, levels, streaks and memory checks, kept only in
  the reader's own browser storage and in files the reader downloads.
- Dependencies: `serde`, `serde_json` and `wasm-bindgen` (WebAssembly only), each pinned to an
  exact version. No unsafe code is written here; the generated binding comes from
  `wasm-bindgen-cli-support` of the same version.

Search input stays in the browser. A shared search link carries its query text, like any link.

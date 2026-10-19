# goroshi-scriptures

The builder of [The Scriptures](../../README.md): it fetches and checks the sources, builds every
page of the reader, crawls the result, and makes the wallpaper pack.

```sh
goroshi-scriptures fetch-redletter    # eBible.org KJV USFM -> data/redletter.json
goroshi-scriptures fetch-art          # Commons API licence gate -> data/art_manifest.json
goroshi-scriptures fetch-originals    # each admitted plate's Commons original, sha1-verified
goroshi-scriptures derive-crisp       # AVIF, WebP and JPEG at every width (cjpeg, cwebp, avifenc)
goroshi-scriptures fetch-lectionary   # orthocal.info -> data/lectionary.json
goroshi-scriptures sources            # writes SOURCES.md
goroshi-scriptures bind-search --wasm scripture_search.wasm --out NEW_DIR
goroshi-scriptures build --search-assets DIR --out NEW_DIR [--edition web|desktop]
goroshi-scriptures verify --out BUILT              # every chapter reachable, no broken link
goroshi-scriptures check-search-wasm --data BUILT  # search identical in Rust and WebAssembly
goroshi-scriptures contact-sheet --data DATA --out BUILT --sheet NEW.html   # every admitted plate, for approval
goroshi-scriptures wallpapers --out NEW_DIR [--sizes 1920x1080,...]
goroshi-scriptures desktop-files      # the desktop art pack's file list
```

Every output directory must be new; nothing is overwritten. `build` refuses any artwork without a
verdict in `data/content_review.tsv` and publishes only `ADMIT`. The scripts in
[`scripts/`](../../scripts) run these in order. Sources and checks: [PROVENANCE.md](../../PROVENANCE.md).

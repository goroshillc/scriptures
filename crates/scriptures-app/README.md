# scriptures-app

The Scriptures as a desktop app for Windows, macOS and Linux (Tauri 2 around the system
webview: WebView2, WKWebView or WebKitGTK).

- The reader's desktop edition is installed beside the app (`site/` in its resources) and served
  through `scriptures-host`'s router on the app's own `scriptures:` scheme: no port, no network.
- Links to other sites open in the system browser; a downloaded reading record goes to the
  Downloads folder and never overwrites a file.
- `scriptures --open /john/3/` opens at any reader path. `--site DIR` (or `SCRIPTURES_SITE`)
  serves another built directory.

Build it with [`scripts/build-app.sh`](../../scripts/build-app.sh) (see the main README).

# scriptures-host

The Scriptures' static host: one built reader directory, read-only.

```sh
goroshi-scriptures-host --root BUILT_DIR --addr 127.0.0.1:PORT   # loopback addresses only
```

GET and HEAD only; dot- and underscore-segments are never served; the build's own
Content-Security-Policy (from `_headers`); immutable caching for content-addressed files; the
request `Cookie` header is dropped before any handler, and no `Set-Cookie` is sent. Put it behind
a reverse proxy or tunnel for a public hostname.

The desktop app uses the same `router` for its private `scriptures:` scheme, so the website and
the app answer every path the same way.

Tests: `cargo test -p scriptures-host`.

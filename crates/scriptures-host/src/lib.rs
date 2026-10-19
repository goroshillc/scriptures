//! The Scriptures' own static host.
//!
//! > *"Freely ye have received, freely give."* - Matthew 10:8, KJV
//!
//! One built reader directory, served read-only on a loopback port, for its own hostname
//! behind a reverse proxy or tunnel. It is deliberately separate from every other system:
//! it shares no process, cookie, session, login, code path or data with any other
//! service, and it links to none of it.
//!
//! What it does, and nothing more:
//!
//! - `GET` and `HEAD` only; anything else is `405`.
//! - Files under the root by validated path; a directory serves its `index.html`. A path
//!   segment beginning with `.` or `_` (the build marker, `_headers`) is never served.
//! - The reader's own Content-Security-Policy, read from the build's `_headers` (it pins
//!   the inline stylesheet by hash and allows WebAssembly compilation, never `eval`).
//! - Long, immutable caching for content-addressed files; short caching for pages.
//! - The browser's `Cookie` header is removed before any handler runs: the reader keeps
//!   its record in a first-party cookie and storage that no server code ever reads.

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower_http::services::ServeDir;

/// Why the host would not start.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// The root is not a built reader directory.
    #[error("{0} is not a built Scriptures directory (no index.html, 404.html and _headers)")]
    NotABuild(PathBuf),
    /// `_headers` carries no Content-Security-Policy.
    #[error("{0}/_headers has no Content-Security-Policy")]
    NoPolicy(PathBuf),
    /// The policy text is not a valid header value.
    #[error("the Content-Security-Policy in _headers is not a valid header value")]
    BadPolicy,
    /// A file could not be read.
    #[error("cannot read {path}: {source}")]
    Read {
        /// The file.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// The listen address is not a loopback address.
    #[error("{0} is not a loopback address; the host listens only behind a proxy")]
    NotLoopback(String),
}

/// What every response carries.
#[derive(Clone, Debug)]
pub struct Policy {
    csp: HeaderValue,
    not_found: Arc<Vec<u8>>,
}

/// Cache lifetime for a site path.
#[must_use]
pub fn cache_for(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or("");
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let hashed = path.starts_with("/art/img/")
        || path.starts_with("/fonts/")
        || (path.starts_with("/discovery/script-") && ext.as_deref() == Some("js"));
    if hashed {
        "public, max-age=31536000, immutable"
    } else if path.ends_with('/') || ext.is_none() || ext.as_deref() == Some("html") {
        "public, max-age=300"
    } else {
        "public, max-age=86400"
    }
}

/// True when every segment of a request path may be served.
#[must_use]
pub fn path_ok(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 512
        && path
            .split('/')
            .all(|segment| !segment.starts_with('.') && !segment.starts_with('_'))
        && !path.contains('\\')
        && !path.contains('\0')
}

/// Reads the Content-Security-Policy the build wrote into `_headers`.
///
/// # Errors
/// Fails when the file is missing or carries no policy.
pub fn read_policy(root: &Path) -> Result<Policy, HostError> {
    for name in ["index.html", "404.html", "_headers"] {
        if !root.join(name).is_file() {
            return Err(HostError::NotABuild(root.to_path_buf()));
        }
    }
    let headers_path = root.join("_headers");
    let headers = std::fs::read_to_string(&headers_path).map_err(|source| HostError::Read {
        path: headers_path,
        source,
    })?;
    let csp = headers
        .lines()
        .find_map(|line| line.trim().strip_prefix("Content-Security-Policy:"))
        .map(str::trim)
        .ok_or_else(|| HostError::NoPolicy(root.to_path_buf()))?;
    let not_found_path = root.join("404.html");
    let not_found = std::fs::read(&not_found_path).map_err(|source| HostError::Read {
        path: not_found_path,
        source,
    })?;
    Ok(Policy {
        csp: HeaderValue::from_str(csp).map_err(|_| HostError::BadPolicy)?,
        not_found: Arc::new(not_found),
    })
}

fn set(headers: &mut HeaderMap, name: header::HeaderName, value: &'static str) {
    headers.insert(name, HeaderValue::from_static(value));
}

fn not_found(policy: &Policy) -> Response {
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        Body::from(policy.not_found.as_ref().clone()),
    )
        .into_response()
}

async fn guard(policy: Policy, mut req: Request, next: Next) -> Response {
    // The reader's record cookie is never read by any server code.
    req.headers_mut().remove(header::COOKIE);
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let mut res = if method != Method::GET && method != Method::HEAD {
        let mut r = (StatusCode::METHOD_NOT_ALLOWED, "GET and HEAD only").into_response();
        r.headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
        r
    } else if path_ok(&path) {
        let res = next.run(req).await;
        if res.status() == StatusCode::NOT_FOUND {
            not_found(&policy)
        } else {
            res
        }
    } else {
        not_found(&policy)
    };
    let h = res.headers_mut();
    h.insert(header::CONTENT_SECURITY_POLICY, policy.csp.clone());
    set(h, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(h, header::X_FRAME_OPTIONS, "DENY");
    set(h, header::REFERRER_POLICY, "no-referrer");
    set(h, header::STRICT_TRANSPORT_SECURITY, "max-age=31536000");
    for (name, value) in [
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-resource-policy", "same-origin"),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=(), interest-cohort=()",
        ),
    ] {
        if let Ok(name) = header::HeaderName::from_bytes(name.as_bytes()) {
            h.insert(name, HeaderValue::from_static(value));
        }
    }
    if h.remove(header::SET_COOKIE).is_some() {
        // Nothing here sets cookies; a stray one is dropped rather than sent.
    }
    let cache = if res.status().is_success() {
        cache_for(&path)
    } else {
        "no-store"
    };
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    res
}

/// The whole host: the built directory behind the guard.
pub fn router(root: &Path, policy: Policy) -> Router {
    let files = ServeDir::new(root).append_index_html_on_directories(true);
    Router::new()
        .fallback_service(files)
        .layer(middleware::from_fn(move |req, next| {
            guard(policy.clone(), req, next)
        }))
}

/// Parses `--root DIR --addr 127.0.0.1:PORT`, refusing any non-loopback address.
///
/// # Errors
/// Fails on a missing value or a non-loopback address.
pub fn parse_addr(addr: &str) -> Result<std::net::SocketAddr, HostError> {
    let parsed: std::net::SocketAddr = addr
        .parse()
        .map_err(|_| HostError::NotLoopback(addr.to_string()))?;
    if !parsed.ip().is_loopback() {
        return Err(HostError::NotLoopback(addr.to_string()));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]
    use super::*;
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    fn site() -> PathBuf {
        // One directory per call: tests run in parallel and must never share files.
        static NTH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nth = NTH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("scriptures-host-test-{}-{nth}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join("john/3"));
        let _ = std::fs::create_dir_all(dir.join("art/img"));
        let _ = std::fs::create_dir_all(dir.join("discovery"));
        std::fs::write(dir.join("index.html"), "<!doctype html><title>Home</title>").unwrap();
        std::fs::write(
            dir.join("404.html"),
            "<!doctype html><title>Not found</title>",
        )
        .unwrap();
        std::fs::write(
            dir.join("_headers"),
            "/*\n  Content-Security-Policy: default-src 'none'; script-src 'self' 'wasm-unsafe-eval'\n",
        )
        .unwrap();
        std::fs::write(dir.join(".goroshi-scriptures-dist"), "marker").unwrap();
        std::fs::write(
            dir.join("john/3/index.html"),
            "<!doctype html><title>John 3</title>",
        )
        .unwrap();
        std::fs::write(dir.join("art/img/plate.0123456789.avif"), b"avif").unwrap();
        std::fs::write(dir.join("discovery/discovery_bg.wasm"), b"\0asm").unwrap();
        dir
    }

    async fn get(method: Method, path: &str) -> Response {
        let root = site();
        let policy = read_policy(&root).unwrap();
        router(&root, policy)
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::COOKIE, "__Host-scriptures=v1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn pages_files_and_wasm_are_served_with_the_readers_own_policy() {
        let page = get(Method::GET, "/john/3/").await;
        assert_eq!(page.status(), StatusCode::OK);
        assert_eq!(
            page.headers()[header::CONTENT_SECURITY_POLICY],
            "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'"
        );
        assert_eq!(page.headers()[header::CACHE_CONTROL], "public, max-age=300");
        assert_eq!(page.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert!(page.headers().get(header::SET_COOKIE).is_none());
        let wasm = get(Method::GET, "/discovery/discovery_bg.wasm").await;
        assert_eq!(wasm.headers()[header::CONTENT_TYPE], "application/wasm");
        assert_eq!(
            wasm.headers()[header::CACHE_CONTROL],
            "public, max-age=86400"
        );
        let avif = get(Method::GET, "/art/img/plate.0123456789.avif").await;
        assert_eq!(avif.headers()[header::CONTENT_TYPE], "image/avif");
        assert_eq!(
            avif.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
    }

    #[tokio::test]
    async fn hidden_files_traversal_and_other_methods_are_refused() {
        for path in [
            "/_headers",
            "/.goroshi-scriptures-dist",
            "/../etc/passwd",
            "/john/%2e%2e/%2e%2e/x",
        ] {
            let res = get(Method::GET, path).await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND, "{path}");
            let body = res.into_body().collect().await.unwrap().to_bytes();
            assert!(
                String::from_utf8_lossy(&body).contains("Not found"),
                "{path}"
            );
        }
        let post = get(Method::POST, "/john/3/").await;
        assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(post.headers()[header::ALLOW], "GET, HEAD");
        assert!(
            post.headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .is_some()
        );
    }

    #[test]
    fn only_loopback_addresses_are_accepted() {
        assert!(parse_addr("127.0.0.1:18810").is_ok());
        assert!(parse_addr("[::1]:18810").is_ok());
        assert!(parse_addr("0.0.0.0:18810").is_err());
        assert!(parse_addr("10.0.0.144:18810").is_err());
    }

    #[test]
    fn caching_follows_content_addressing() {
        assert_eq!(
            cache_for("/fonts/EBGaramond-Text.d5bf588278.woff"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(
            cache_for("/discovery/script-0123456789abcdef.js"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_for("/discovery/reader.js"), "public, max-age=86400");
        assert_eq!(cache_for("/"), "public, max-age=300");
        assert_eq!(cache_for("/profile/index.html"), "public, max-age=300");
    }
}

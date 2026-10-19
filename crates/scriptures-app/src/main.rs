//! The Scriptures as a desktop app: the same reader, offline, on Windows, macOS and Linux.
//!
//! > *"Thy word is a lamp unto my feet, and a light unto my path."* - Psalms 119:105, KJV
//!
//! WHY a private scheme and not a local server: the app answers its own `scriptures:`
//! scheme inside the webview, so it opens no port, needs no network and cannot be reached
//! by any other program. Every request goes through the reader's own host
//! ([`scriptures_host::router`]): the same path rules, Content-Security-Policy and file
//! serving the website uses, reading the built desktop edition installed beside the app.
//!
//! The reader's private record stays where the website keeps it, in the webview's own
//! storage on this computer; a downloaded progress file goes to the Downloads folder.
//! Links that leave the reader (eBible.org, Wikimedia Commons, parish directories) open in
//! the system browser, never inside the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use axum::body::Body;
use http_body_util::BodyExt as _;
use std::path::{Path, PathBuf};
use tauri::http::{Request, Response, StatusCode, header};
use tauri::webview::DownloadEvent;
use tauri::{Manager as _, Url, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt as _;
use tower::ServiceExt as _;

/// The app's private URL scheme.
const SCHEME: &str = "scriptures";

/// Where the scheme's pages live: `scriptures://localhost` on macOS and Linux, and
/// `https://scriptures.localhost` on Windows (the WebView2 form, kept on https so the
/// origin, and the reader's record stored under it, never changes between launches).
#[cfg(any(windows, target_os = "android"))]
const ORIGIN: &str = "https://scriptures.localhost";
#[cfg(not(any(windows, target_os = "android")))]
const ORIGIN: &str = "scriptures://localhost";

/// The built reader the app serves, or why there is none.
struct Site {
    router: Result<axum::Router, String>,
}

/// True for a URL the reader itself serves (or a download it made); everything else is
/// opened in the system browser.
fn is_local(url: &Url) -> bool {
    match url.scheme() {
        SCHEME | "blob" | "data" | "about" => true,
        "http" | "https" => url.host_str() == Some("scriptures.localhost"),
        _ => false,
    }
}

/// The page to open first: `--open /john/3/` (any reader path), else the home page.
fn start_path(args: &[String]) -> String {
    let wanted = args
        .iter()
        .position(|a| a == "--open")
        .and_then(|i| args.get(i + 1))
        .filter(|p| p.starts_with('/') && !p.starts_with("//") && p.len() <= 512)
        .filter(|p| !p.contains(['\\', '\0', '#']));
    wanted.cloned().unwrap_or_else(|| "/".to_string())
}

/// The built site: `--site DIR`, else `SCRIPTURES_SITE`, else `site/` in the app's
/// resource directory.
fn site_dir(args: &[String], resources: Option<PathBuf>) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == "--site")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("SCRIPTURES_SITE").map(PathBuf::from))
        .or_else(|| resources.map(|r| r.join("site")))
}

fn load_site(dir: Option<PathBuf>) -> Site {
    let router = dir
        .ok_or_else(|| "no resource directory".to_string())
        .and_then(|dir| {
            let dir = dir
                .canonicalize()
                .map_err(|e| format!("{}: {e}", dir.display()))?;
            let policy = scriptures_host::read_policy(&dir).map_err(|e| e.to_string())?;
            Ok(scriptures_host::router(&dir, policy))
        });
    Site { router }
}

/// A file name in `dir` that does not exist yet: `name`, else `stem (2).ext`, and so on.
fn unique_destination(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let ext = path.extension().and_then(|s| s.to_str());
    (2..10_000)
        .map(|n| {
            dir.join(ext.map_or_else(|| format!("{stem} ({n})"), |e| format!("{stem} ({n}).{e}")))
        })
        .find(|p| !p.exists())
        .unwrap_or(first)
}

fn plain(status: StatusCode, text: String) -> Response<Vec<u8>> {
    let mut res = Response::new(text.into_bytes());
    *res.status_mut() = status;
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    res
}

async fn once(router: &axum::Router, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let (parts, body) = request.into_parts();
    let request = axum::http::Request::from_parts(parts, Body::from(body));
    let Ok(response) = router.clone().oneshot(request).await;
    let (parts, body) = response.into_parts();
    match body.collect().await {
        Ok(collected) => Response::from_parts(parts, collected.to_bytes().to_vec()),
        Err(e) => plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("read failed: {e}"),
        ),
    }
}

/// Answers one request of the app's scheme through the reader's host.
///
/// A directory asked for without its trailing slash is answered with its page directly:
/// the webview does not follow redirects on a private scheme.
async fn serve(site: &Site, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let router = match &site.router {
        Ok(router) => router,
        Err(why) => {
            return plain(
                StatusCode::SERVICE_UNAVAILABLE,
                format!(
                    "The Scriptures could not find its pages ({why}).\n\
                     Reinstall it from https://github.com/goroshillc/scriptures/releases\n"
                ),
            );
        }
    };
    let method = request.method().clone();
    let uri = request.uri().clone();
    let response = once(router, request).await;
    // The host's redirect names the same page with its slash, absolute or root-relative.
    let location = response
        .headers()
        .get(header::LOCATION)
        .filter(|_| response.status().is_redirection())
        .and_then(|v| v.to_str().ok())
        .and_then(|l| l.parse::<tauri::http::Uri>().ok())
        .filter(|l| l.authority().is_none() || l.authority() == uri.authority())
        .and_then(|l| l.path_and_query().map(|pq| pq.as_str().to_string()))
        .filter(|p| p.starts_with('/') && !p.starts_with("//"));
    let Some(location) = location else {
        return response;
    };
    let target = format!(
        "{}://{}{location}",
        uri.scheme_str().unwrap_or(SCHEME),
        uri.authority().map_or("localhost", |a| a.as_str())
    );
    let Ok(again) = Request::builder()
        .method(method)
        .uri(target)
        .body(Vec::new())
    else {
        return response;
    };
    once(router, again).await
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let start = start_path(&args);
    let run = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol(SCHEME, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let response = match app.try_state::<Site>() {
                    Some(site) => serve(&site, request).await,
                    None => plain(StatusCode::SERVICE_UNAVAILABLE, "starting\n".to_string()),
                };
                responder.respond(response);
            });
        })
        .setup(move |app| {
            let resources = app.path().resource_dir().ok();
            app.manage(load_site(site_dir(&args, resources)));
            let url: Url = format!("{ORIGIN}{start}").parse()?;
            let opener = app.handle().clone();
            let downloads = app.path().download_dir().ok();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::CustomProtocol(url))
                .title("The Scriptures")
                .inner_size(1200.0, 900.0)
                .min_inner_size(400.0, 560.0)
                .use_https_scheme(true)
                .zoom_hotkeys_enabled(true)
                .on_document_title_changed(|window, title| {
                    let _ = window.set_title(&title);
                })
                .on_navigation(move |url| {
                    if is_local(url) {
                        return true;
                    }
                    if matches!(url.scheme(), "http" | "https" | "mailto") {
                        let _ = opener.opener().open_url(url.as_str(), None::<&str>);
                    }
                    false
                })
                .on_download(move |_webview, event| {
                    if let DownloadEvent::Requested { destination, .. } = event {
                        let name = destination
                            .file_name()
                            .and_then(|n| n.to_str())
                            .filter(|n| !n.is_empty())
                            .unwrap_or("scriptures-download")
                            .to_string();
                        if let Some(dir) = &downloads {
                            *destination = unique_destination(dir, &name);
                        }
                    }
                    true
                })
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(e) = run {
        eprintln!("The Scriptures could not start: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    fn site() -> PathBuf {
        static NTH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nth = NTH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("scriptures-app-test-{}-{nth}", std::process::id()));
        std::fs::create_dir_all(dir.join("john/3")).unwrap();
        std::fs::write(dir.join("index.html"), "<title>Home</title>").unwrap();
        std::fs::write(dir.join("404.html"), "<title>Not found</title>").unwrap();
        std::fs::write(dir.join("john/3/index.html"), "<title>John 3</title>").unwrap();
        std::fs::write(
            dir.join("_headers"),
            "/*\n  Content-Security-Policy: default-src 'none'\n",
        )
        .unwrap();
        dir
    }

    fn get(path: &str) -> Request<Vec<u8>> {
        Request::builder()
            .uri(format!("scriptures://localhost{path}"))
            .body(Vec::new())
            .unwrap()
    }

    #[test]
    fn only_the_readers_own_pages_stay_in_the_window() {
        for local in [
            "scriptures://localhost/john/3/",
            "https://scriptures.localhost/john/3/",
            "blob:scriptures://localhost/1f2e",
            "about:blank",
        ] {
            assert!(is_local(&local.parse().unwrap()), "{local}");
        }
        for outside in [
            "https://ebible.org/kjv/",
            "https://commons.wikimedia.org/wiki/File:x.jpg",
            "http://localhost:8080/",
            "file:///etc/passwd",
        ] {
            assert!(!is_local(&outside.parse().unwrap()), "{outside}");
        }
    }

    #[test]
    fn the_first_page_is_home_unless_a_reader_path_is_named() {
        assert_eq!(start_path(&args(&["scriptures"])), "/");
        assert_eq!(
            start_path(&args(&["scriptures", "--open", "/search/?q=light"])),
            "/search/?q=light"
        );
        assert_eq!(
            start_path(&args(&["scriptures", "--open", "//evil.example/"])),
            "/"
        );
        assert_eq!(
            start_path(&args(&["scriptures", "--open", "https://x/"])),
            "/"
        );
        assert_eq!(start_path(&args(&["scriptures", "--open"])), "/");
    }

    #[test]
    fn the_site_is_found_by_flag_then_resources() {
        assert_eq!(
            site_dir(
                &args(&["s", "--site", "/tmp/built"]),
                Some(PathBuf::from("/r"))
            ),
            Some(PathBuf::from("/tmp/built"))
        );
        if std::env::var_os("SCRIPTURES_SITE").is_none() {
            assert_eq!(
                site_dir(&args(&["s"]), Some(PathBuf::from("/r"))),
                Some(PathBuf::from("/r/site"))
            );
        }
    }

    #[test]
    fn a_download_never_overwrites_an_existing_file() {
        let dir = site();
        assert_eq!(
            unique_destination(&dir, "progress.json"),
            dir.join("progress.json")
        );
        std::fs::write(dir.join("progress.json"), "{}").unwrap();
        assert_eq!(
            unique_destination(&dir, "progress.json"),
            dir.join("progress (2).json")
        );
    }

    #[test]
    fn pages_come_from_the_readers_host_with_its_policy() {
        let s = load_site(Some(site()));
        let res = tauri::async_runtime::block_on(serve(&s, get("/john/3/")));
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.body().as_slice(), b"<title>John 3</title>");
        assert_eq!(
            res.headers().get(header::CONTENT_SECURITY_POLICY).unwrap(),
            "default-src 'none'"
        );
        let hidden = tauri::async_runtime::block_on(serve(&s, get("/_headers")));
        assert_eq!(hidden.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn a_directory_without_its_slash_is_answered_directly() {
        let s = load_site(Some(site()));
        let res = tauri::async_runtime::block_on(serve(&s, get("/john/3")));
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.body().as_slice(), b"<title>John 3</title>");
    }

    #[test]
    fn a_missing_site_says_how_to_fix_it() {
        let s = load_site(Some(PathBuf::from("/nonexistent/scriptures-site")));
        let res = tauri::async_runtime::block_on(serve(&s, get("/")));
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(String::from_utf8_lossy(res.body()).contains("/releases"));
    }
}

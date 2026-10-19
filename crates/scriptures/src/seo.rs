//! Discoverability files: `sitemap.xml` (with image entries), `robots.txt`, and the
//! `_headers` file that carries the security and caching policy.
//!
//! > *"What I tell you in darkness, that speak ye in light: and what ye hear in the ear,
//! > that preach ye upon the housetops."* - Matthew 10:27, KJV
//!
//! WHY `_headers`: Netlify and Cloudflare Pages read it natively, the preview server
//! applies the same file, and its CSP pins the one inline stylesheet and script by
//! hash - so what is measured locally is what would be served.

use crate::html;
use crate::meta::xml_escape;
use crate::site::SiteConfig;
use std::fmt::Write as _;

/// One sitemap entry.
#[derive(Debug, Clone)]
pub struct UrlEntry {
    /// Site-relative path.
    pub path: String,
    /// Absolute image URLs shown on the page (image sitemap extension).
    pub images: Vec<String>,
}

/// Builds `sitemap.xml` for every indexable page.
#[must_use]
pub fn sitemap(cfg: &SiteConfig, entries: &[UrlEntry]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:image=\"http://www.google.com/schemas/sitemap-image/1.1\">\n",
    );
    for e in entries {
        let _ = write!(
            s,
            "<url><loc>{}</loc><lastmod>{}</lastmod>",
            xml_escape(&cfg.abs(&e.path)),
            cfg.build_date
        );
        for img in &e.images {
            let _ = write!(
                s,
                "<image:image><image:loc>{}</image:loc></image:image>",
                xml_escape(img)
            );
        }
        s.push_str("</url>\n");
    }
    s.push_str("</urlset>\n");
    s
}

/// Builds `robots.txt`.
#[must_use]
pub fn robots(cfg: &SiteConfig) -> String {
    format!(
        "User-agent: *\nAllow: /\n\nSitemap: {}\n",
        cfg.abs("sitemap.xml")
    )
}

/// The Content-Security-Policy for every page.
///
/// It pins the shared stylesheet and script and any page scripts by hash, and allows
/// `'wasm-unsafe-eval'` so the Paschalion's WebAssembly can compile (no `eval` of
/// JavaScript is allowed).
#[must_use]
pub fn csp(cfg: &SiteConfig, extra_scripts: &[String]) -> String {
    let mut scripts = vec![
        html::csp_hash(html::JS),
        "'self'".into(),
        "'wasm-unsafe-eval'".into(),
    ];
    scripts.extend(extra_scripts.iter().map(|s| html::csp_hash(s)));
    format!(
        "default-src 'none'; img-src 'self' data:; font-src 'self'; style-src {} {}; script-src {}; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
        html::csp_hash(&html::stylesheet(cfg)),
        // The favicon's own light and dark rule; the host sends this policy with the icon too.
        html::csp_hash(chi_rho::FAVICON_STYLE),
        scripts.join(" ")
    )
}

/// Builds `_headers` (Netlify / Cloudflare Pages format).
#[must_use]
pub fn headers(cfg: &SiteConfig, extra_scripts: &[String]) -> String {
    let base = cfg.base_path.trim_end_matches('/');
    format!(
        concat!(
            "{base}/*\n",
            "  Content-Security-Policy: {csp}\n",
            "  X-Content-Type-Options: nosniff\n",
            "  Referrer-Policy: strict-origin-when-cross-origin\n",
            "  Permissions-Policy: camera=(), microphone=(), geolocation=(), interest-cohort=()\n",
            "  Cross-Origin-Opener-Policy: same-origin\n",
            "  Cache-Control: public, max-age=600\n",
            "{base}/art/img/*\n",
            "  Cache-Control: public, max-age=31536000, immutable\n",
            "{base}/fonts/*\n",
            "  Cache-Control: public, max-age=31536000, immutable\n"
        ),
        base = base,
        csp = csp(cfg, extra_scripts)
    )
}

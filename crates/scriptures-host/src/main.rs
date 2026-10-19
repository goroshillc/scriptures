//! `goroshi-scriptures-host --root BUILT_DIR --addr 127.0.0.1:PORT`
//!
//! Serves one built Scriptures directory on a loopback address, for its own hostname
//! behind a reverse proxy or tunnel. See the library documentation for exactly what it does.

use std::path::PathBuf;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut root, mut addr) = (None, None);
    while let Some(flag) = args.next() {
        match (flag.as_str(), args.next()) {
            ("--root", Some(value)) => root = Some(PathBuf::from(value)),
            ("--addr", Some(value)) => addr = Some(value),
            _ => {
                eprintln!("usage: goroshi-scriptures-host --root BUILT_DIR --addr 127.0.0.1:PORT");
                return std::process::ExitCode::FAILURE;
            }
        }
    }
    let (Some(root), Some(addr)) = (root, addr) else {
        eprintln!("usage: goroshi-scriptures-host --root BUILT_DIR --addr 127.0.0.1:PORT");
        return std::process::ExitCode::FAILURE;
    };
    match run(root, &addr).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run(root: PathBuf, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let addr = scriptures_host::parse_addr(addr)?;
    let root = root.canonicalize()?;
    let policy = scriptures_host::read_policy(&root)?;
    let app = scriptures_host::router(&root, policy);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "goroshi-scriptures-host serving {} on http://{addr} (loopback only)",
        root.display()
    );
    axum::serve(listener, app).await?;
    Ok(())
}

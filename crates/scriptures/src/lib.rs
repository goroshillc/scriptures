//! # The Scriptures - a static King James reader for the open web
//!
//! > *"Therefore whosoever heareth these sayings of mine, and doeth them, I will liken him
//! > unto a wise man, which built his house upon a rock:"* - Matthew 7:24, KJV
//!
//! > *"I am the vine, ye are the branches: He that abideth in me, and I in him, the same
//! > bringeth forth much fruit: for without me ye can do nothing."* - John 15:5, KJV
//!
//! Both red-letter verses are quoted verbatim from the on-disk KJV (`data/kjv.json`).
//! JESUS CHRIST is the cornerstone and chief builder of this work, and its source and
//! inspiration: all of it is from Him. Amen.
//!
//! ## What this crate is
//!
//! A Rust port of an earlier TypeScript reader (`lib/scripture.ts`) into a static-site
//! generator, so the reader can be published without a Node server:
//!
//! - [`bible`] and [`xref`] port that engine (`lib/scripture.ts`) line for
//!   line; `tests/parity.rs` proves the same verse text and the same resolved
//!   cross-reference sets against the original engine's own output.
//! - [`redletter`] derives the words of Christ from the eBible.org red-letter KJV
//!   markup (`\wj`), aligned word by word onto the reader's text.
//! - [`art`] holds the public-domain master art (Doré, Dürer, Titian) fetched from
//!   Wikimedia Commons with the licence read per file from the Commons API.
//! - [`meta`] writes XMP and IPTC into every served JPEG: credit, truthful
//!   public-domain status, the site's own notice, and KJV verses.
//! - [`render`], [`seo`] and [`site`] write the pages, sitemap, robots and headers.
//! - [`contact`] writes the contact sheet of every admitted plate for a maintainer's approval.

pub mod art;
pub mod bible;
pub mod canon;
pub mod commons;
pub mod contact;
pub mod crisp;
pub mod discovery;
pub mod error;
pub mod features;
pub mod gallery;
pub mod html;
pub mod imaging;
pub mod lectionary;
pub mod meta;
pub mod net;
pub mod pages;
pub mod redletter;
pub mod render;
pub mod seo;
pub mod site;
pub mod verify;
pub mod wallpaper;
pub mod wasm_check;
pub mod xref;

pub use error::Error;

/// Result alias used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

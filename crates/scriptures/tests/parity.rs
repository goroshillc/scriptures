//! Parity: the Rust port against the original TypeScript engine.
//!
//! The fixture `tests/fixtures/engine_parity.json` was produced once by running the
//! ORIGINAL `lib/scripture.ts` the reader was first written in. These tests
//! prove the port shows the same verse text and the same resolved cross-reference sets:
//! field by field for a sample of 336 verses, and by sha256 over all 31,102 verses.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use goroshi_scriptures::bible::Bible;
use goroshi_scriptures::canon;
use goroshi_scriptures::xref::{self, Xrefs};
use serde_json::Value;
use std::path::Path;

fn load() -> (Bible, Xrefs, Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bible = Bible::load(&root.join("data/kjv.json")).expect("kjv.json loads");
    let xrefs = Xrefs::load(&root.join("data/xrefs.json")).expect("xrefs.json loads");
    let raw = std::fs::read_to_string(root.join("tests/fixtures/engine_parity.json"))
        .expect("fixture present");
    (
        bible,
        xrefs,
        serde_json::from_str(&raw).expect("fixture is JSON"),
    )
}

fn all_keys(bible: &Bible) -> Vec<String> {
    let mut keys = Vec::new();
    for b in 0..canon::BOOKS.len() {
        for c in 1..=bible.chapter_count(b) {
            for v in 1..=bible.verse_count(b, c) {
                keys.push(xref::osis_key(b, c, v));
            }
        }
    }
    keys
}

#[test]
fn the_corpus_has_66_books_1189_chapters_and_31102_verses_like_the_original() {
    let (bible, _, fx) = load();
    assert_eq!(fx["stats"]["books"], 66);
    assert_eq!(fx["stats"]["chapters"].as_u64(), Some(1189));
    assert_eq!(fx["stats"]["verses"].as_u64(), Some(31102));
    assert_eq!(bible.total_chapters(), 1189);
    assert_eq!(bible.total_verses(), 31102);
}

#[test]
fn every_book_has_the_same_code_name_and_chapter_count_as_the_original() {
    let (bible, _, fx) = load();
    let books = fx["books"].as_array().expect("books array");
    assert_eq!(books.len(), 66);
    for (i, b) in books.iter().enumerate() {
        assert_eq!(
            b["code"].as_str(),
            Some(canon::BOOKS[i].osis),
            "book {i} code"
        );
        assert_eq!(b["name"].as_str(), bible.book_name(i), "book {i} name");
        assert_eq!(
            b["chapters"].as_u64(),
            u64::try_from(bible.chapter_count(i)).ok(),
            "book {i} chapters"
        );
    }
}

#[test]
fn sampled_verses_match_the_original_text_and_cross_reference_sets_exactly() {
    let (bible, xrefs, fx) = load();
    let sample = fx["sample"].as_array().expect("sample array");
    assert_eq!(sample.len(), 336);
    let mut refs_checked = 0usize;
    for want in sample {
        let osis = want["osis"].as_str().expect("osis");
        let got = xref::get_verse(&bible, &xrefs, osis).expect("verse exists");
        let got = serde_json::to_value(&got).expect("serialises");
        assert_eq!(got["text"], want["text"], "{osis} text");
        assert_eq!(got["ref"], want["ref"], "{osis} human reference");
        let w = want["crossRefs"].as_array().expect("crossRefs");
        let g = got["crossRefs"].as_array().expect("crossRefs");
        assert_eq!(g.len(), w.len(), "{osis} cross-reference count");
        for (gx, wx) in g.iter().zip(w) {
            assert_eq!(gx, wx, "{osis} cross-reference");
        }
        refs_checked += w.len();
    }
    assert!(
        refs_checked > 3000,
        "sample exercises {refs_checked} resolved references"
    );
}

#[test]
fn john_3_16_resolves_its_23_cross_references_with_romans_5_8_first() {
    let (bible, xrefs, _) = load();
    let v = xref::get_verse(&bible, &xrefs, "John.3.16").expect("John 3:16");
    assert_eq!(v.cross_refs.len(), 23);
    assert_eq!(v.cross_refs[0].reference, "Romans 5:8");
    assert!(v.text.starts_with("For God so loved the world"));
}

#[test]
fn the_whole_corpus_hashes_identically_to_the_original_engine() {
    let (bible, xrefs, fx) = load();
    let keys = all_keys(&bible);
    let mut corpus = Vec::new();
    for k in &keys {
        let v = xref::get_verse(&bible, &xrefs, k);
        corpus.extend_from_slice(serde_json::to_string(&v).expect("serialises").as_bytes());
        corpus.push(b'\n');
    }
    let hex = goroshi_scriptures::site::sha256_hex(&corpus);
    assert_eq!(keys.len(), 31102);
    assert_eq!(fx["corpus"]["verses"].as_u64(), Some(31102));
    assert_eq!(
        Some(hex.as_str()),
        fx["corpus"]["sha256"].as_str(),
        "whole-corpus digest"
    );
}

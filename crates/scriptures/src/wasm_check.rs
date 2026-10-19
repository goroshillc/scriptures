//! Exercise generated JavaScript/WASM bindings without a browser or any network.

use crate::{Error, Result};
use scripture_search::{Document, Index, Query};
use std::path::Path;

// The small JavaScript host is the binding boundary under test, not a second
// search implementation. All matching and ranking executes in compiled Rust.
const PROBE: &str = r"
import fs from 'node:fs';
import {pathToFileURL} from 'node:url';
const [binding, wasm, catalogue, queries, events] = process.argv.slice(1);
const engine = await import(pathToFileURL(binding).href);
engine.initSync({module: fs.readFileSync(wasm)});
engine.initialize(fs.readFileSync(catalogue, 'utf8'));
const result = JSON.parse(queries).map(query => JSON.parse(engine.search(JSON.stringify(query))));
const marks = JSON.parse(engine.mark('For God so loved the world, that he gave his only begotten Son', 'LOVED world', false));
const passages = ['Jn 3:16', 'ps 23', 'John', 'love one another'].map(q => JSON.parse(engine.passage(q)));
let store = engine.progress_new('reader-1', 'Reader');
const credits = [];
for (const event of JSON.parse(events)) {
  const out = JSON.parse(engine.progress_apply(store, JSON.stringify(event), '2026-10-19', 0));
  store = JSON.stringify(out.store);
  credits.push(out.credit);
}
const view = JSON.parse(engine.progress_view(store, '2026-10-19'));
const file = engine.progress_export(store, 'reader-1');
const other = engine.progress_import(engine.progress_new('reader-2', 'Other'), file);
let refused = 0;
for (const bad of ['{}', file.slice(0, file.length / 2), file.replace('john', 'thomas')]) {
  try { engine.progress_import(other, bad); } catch (_) { refused += 1; }
}
process.stdout.write(JSON.stringify({result, marks, passages, credits, view, imported: JSON.parse(other).profiles.length, refused}));
";

/// Compare real built WASM results with the same native Rust engine.
///
/// # Errors
/// Rejects absent assets, invalid input or any mismatch in result identities.
#[allow(clippy::too_many_lines)]
pub fn verify(build: &Path) -> Result<serde_json::Value> {
    let catalogue = build.join("discovery/catalogue.json");
    let bytes = std::fs::read(&catalogue)
        .map_err(|error| Error::io("catalogue parity", &catalogue, error))?;
    let docs: Vec<Document> =
        serde_json::from_slice(&bytes).map_err(|error| Error::json("catalogue parity", error))?;
    let count = docs.len();
    let index = Index::new(docs).map_err(Error::Data)?;
    let queries = vec![
        Query {
            q: "John3:16".into(),
            kind: "verse".into(),
            ..Query::default()
        },
        Query {
            q: "dore".into(),
            kind: "art".into(),
            ..Query::default()
        },
        Query {
            topic: "resurrection".into(),
            kind: "verse".into(),
            ..Query::default()
        },
        Query {
            book: "Not a canonical book".into(),
            ..Query::default()
        },
        Query {
            kind: "verse".into(),
            offset: 40,
            ..Query::default()
        },
        Query {
            q: "loved the world".into(),
            testament: "New Testament".into(),
            ..Query::default()
        },
        Query {
            kind: "verse".into(),
            with_art: true,
            ..Query::default()
        },
        Query {
            q: "verily".into(),
            red_letter: true,
            book: "John".into(),
            ..Query::default()
        },
        Query {
            q: "shepherd pastures".into(),
            mode: "any".into(),
            ..Query::default()
        },
        Query {
            q: "the way, the truth".into(),
            mode: "phrase".into(),
            ..Query::default()
        },
        Query {
            q: "beleive".into(),
            fuzzy: true,
            ..Query::default()
        },
        Query {
            q: "show me".into(),
            kind: "verse".into(),
            ..Query::default()
        },
        Query {
            q: "show me".into(),
            kind: "verse".into(),
            exact_spelling: true,
            ..Query::default()
        },
        Query {
            q: "Jn 3".into(),
            ..Query::default()
        },
    ];
    let expected: Vec<_> = queries.iter().map(|query| index.search(query)).collect();
    if expected.first().is_none_or(|result| result.total != 1)
        || expected.get(1).is_none_or(|result| result.total == 0)
        || expected.get(2).is_none_or(|result| result.total == 0)
        || expected.get(3).is_none_or(|result| result.total != 0)
        || expected.get(5).is_none_or(|result| {
            result
                .marks
                .first()
                .is_none_or(|marks| marks.text.is_empty())
        })
        || expected.get(6).is_none_or(|result| result.total == 0)
        || expected.get(7).is_none_or(|result| result.total == 0)
        || expected.get(8).is_none_or(|result| result.total == 0)
        || expected.get(9).is_none_or(|result| result.total == 0)
        || expected
            .get(10)
            .is_none_or(|result| result.near.is_empty() || !result.fuzzy.iter().any(|f| *f))
        || expected
            .get(11)
            .zip(expected.get(12))
            .is_none_or(|(kjv, exact)| kjv.total <= exact.total || kjv.variants.is_empty())
        || expected
            .get(13)
            .is_none_or(|result| result.jump.as_ref().is_none_or(|j| !j.passage))
    {
        return Err(Error::Data(
            "Built catalogue failed reference/art/topic/empty/highlight/with-art/red-letter/any/phrase/near-spelling/KJV-spelling/jump control".into(),
        ));
    }
    let raw =
        serde_json::to_string(&queries).map_err(|error| Error::json("parity queries", error))?;
    let events = serde_json::json!([
        {"kind": "chapter", "book": "john", "chapter": 3, "verses": 36, "words": 900, "visible_ms": 30_000, "reached_end": true},
        {"kind": "chapter", "book": "john", "chapter": 3, "verses": 36, "words": 900, "visible_ms": 200_000, "reached_end": true},
        {"kind": "chapter", "book": "john", "chapter": 3, "verses": 36, "words": 900, "visible_ms": 200_000, "reached_end": true},
        {"kind": "prayer", "rule": "morning"},
        {"kind": "repetitions", "count": 100},
        {"kind": "xref", "target": "john/3/#v16"},
        {"kind": "plate", "slug": "the-creation-of-light", "visible_ms": 9_000},
        {"kind": "keep", "reference": "John 11:35", "href": "/john/11/#v35", "text": "Jesus wept."},
        {"kind": "check", "reference": "John 11:35", "answers": ["Jesus"]}
    ]);
    let native_progress = {
        use scripture_search::progress::{self, Event, Store};
        let mut store = Store::new("reader-1", "Reader").map_err(|e| Error::Data(e.0))?;
        let mut credits = Vec::new();
        let list: Vec<Event> = serde_json::from_value(events.clone())
            .map_err(|error| Error::json("parity events", error))?;
        for event in &list {
            credits.push(
                store
                    .apply(event, "2026-10-19", 0)
                    .map_err(|e| Error::Data(e.0))?,
            );
        }
        let view = progress::view(&store, "2026-10-19").map_err(|e| Error::Data(e.0))?;
        let file = progress::export(&store, "reader-1").map_err(|e| Error::Data(e.0))?;
        let mut other = Store::new("reader-2", "Other").map_err(|e| Error::Data(e.0))?;
        other.import(&file).map_err(|e| Error::Data(e.0))?;
        let half = file.get(..file.len() / 2).unwrap_or("");
        let refused = ["{}", half, &file.replace("john", "thomas")]
            .iter()
            .filter(|bad| other.clone().import(bad).is_err())
            .count();
        (
            serde_json::to_value(credits).map_err(|error| Error::json("credits", error))?,
            serde_json::to_value(view).map_err(|error| Error::json("view", error))?,
            other.profiles.len(),
            refused,
        )
    };
    if native_progress.3 != 3 || native_progress.2 != 2 {
        return Err(Error::Data(
            "Progress control failed: a corrupted or foreign file was accepted, or a good one refused".into(),
        ));
    }
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "-e", PROBE])
        .arg(build.join("discovery/discovery.js"))
        .arg(build.join("discovery/discovery_bg.wasm"))
        .arg(&catalogue)
        .arg(raw)
        .arg(events.to_string())
        .output()
        .map_err(|error| Error::io("node binding probe", build, error))?;
    if !output.status.success() {
        return Err(Error::Data(format!(
            "WASM binding probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let actual: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| Error::json("WASM output", error))?;
    let passages = ["Jn 3:16", "ps 23", "John", "love one another"]
        .iter()
        .map(|q| serde_json::from_str::<serde_json::Value>(&scripture_search::passage_json(q)))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| Error::json("passages", error))?;
    let expected = serde_json::json!({
        "result": serde_json::to_value(expected)
            .map_err(|error| Error::json("native output", error))?,
        "marks": scripture_search::highlight(
            "For God so loved the world, that he gave his only begotten Son",
            "LOVED world",
            false
        ),
        "passages": passages,
        "credits": native_progress.0,
        "view": native_progress.1,
        "imported": native_progress.2,
        "refused": native_progress.3,
    });
    if actual != expected {
        return Err(Error::Data("Native/WASM result mismatch".into()));
    }
    Ok(
        serde_json::json!({"documents":count,"queries":queries.len(),"progress_events":events.as_array().map_or(0, Vec::len),"native_wasm_parity":"PASS","results":actual,"browser_visual":"NOT_RUN","live_host":"NOT_RUN"}),
    )
}

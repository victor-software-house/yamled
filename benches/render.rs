//! How long the writer takes to choose a string's style, and to rewrite a
//! whole ledger on one line per string in one batch.
//!
//! The ledger is `tests/fixtures/ledger.yaml`, or the file
//! `YAMLED_BENCH_LEDGER` names, such as a large consumer ledger.

use std::hint::black_box;
use std::{env, fs};

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use serde_json::Value;
use yamled::bench::line;
use yamled::{Document, Error, Path, Quotes, TextStyle};

/// Every string scalar of the ledger with its path, keys excluded.
fn strings(value: &Value, path: &Path, found: &mut Vec<(Path, String)>) {
    match value {
        Value::String(text) => found.push((path.clone(), text.clone())),
        Value::Array(items) => {
            for (at, item) in items.iter().enumerate() {
                strings(item, &path.clone().index(at), found);
            }
        }
        Value::Object(fields) => {
            for (key, field) in fields {
                strings(field, &path.clone().key(key), found);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its ledger cannot run"
)]
fn ledger() -> (String, Vec<(Path, String)>) {
    let file = env::var("YAMLED_BENCH_LEDGER")
        .unwrap_or_else(|_| format!("{}/tests/fixtures/ledger.yaml", env!("CARGO_MANIFEST_DIR")));
    let source = fs::read_to_string(&file).expect("read the ledger");
    let value: Value = serde_saphyr::from_str(&source).expect("parse the ledger");
    let mut found = Vec::new();
    strings(&value, &Path::root(), &mut found);
    (source, found)
}

fn rewrite(document: &mut Document, items: &[(Path, String)]) -> Result<(), Error> {
    document.batch(|batch| {
        for (path, text) in items {
            batch.replace_text(path, text, TextStyle::Line)?;
        }
        Ok(())
    })
}

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its ledger cannot run"
)]
fn benches(c: &mut Criterion) {
    let (source, items) = ledger();
    let texts: Vec<&str> = items.iter().map(|(_, text)| text.as_str()).collect();

    let mut decide = c.benchmark_group("decide");
    decide.throughput(Throughput::Elements(texts.len() as u64));
    for (name, flow) in [("block", false), ("flow", true)] {
        decide.bench_function(name, |b| {
            b.iter(|| {
                for text in &texts {
                    black_box(line(black_box(text), Quotes::Auto, flow));
                }
            });
        });
    }
    decide.finish();

    let document = Document::parse(source).expect("index the ledger");
    let mut rewritten = document.clone();
    rewrite(&mut rewritten, &items).expect("rewrite the ledger");
    let mut ledger = c.benchmark_group("ledger");
    ledger.throughput(Throughput::Elements(items.len() as u64));
    for (name, start) in [("rewrite", &document), ("unchanged", &rewritten)] {
        ledger.bench_function(name, |b| {
            b.iter_batched(
                || start.clone(),
                |mut document| rewrite(&mut document, &items).expect("rewrite the ledger"),
                BatchSize::LargeInput,
            );
        });
    }
    ledger.finish();
}

criterion_group!(render, benches);
criterion_main!(render);

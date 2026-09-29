//! The public YAML test suite, pinned as the `tests/yaml-test-suite`
//! submodule. Every input yamled parses survives a replace of each scalar
//! with its own value: the edit keeps every byte outside that node and the
//! document's value. Every input it refuses fails with an error, not a panic.
#![allow(missing_docs)]

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use yamled::{Document, Error, Path};

/// How many scalars the run replaced at the pinned suite commit. Fewer means
/// an edit started refusing inputs it handled; raise it when a fix handles
/// more.
const REPLACED_AT_PIN: usize = 534;

/// What the corpus run saw, printed so a drop in coverage is visible.
#[derive(Debug, Default)]
struct Tally {
    inputs: usize,
    refused: usize,
    unreadable: usize,
    replaced: usize,
    unsupported: usize,
}

/// Every `in.yaml` under the suite, in a stable order. The suite's `name`
/// and `tags` folders are links back to the same cases.
fn inputs() -> Vec<PathBuf> {
    let suite = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/yaml-test-suite");
    let mut found = Vec::new();
    let mut pending = vec![suite.clone()];
    while let Some(dir) = pending.pop() {
        let entries =
            fs::read_dir(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
                .path();
            let top_index = dir == suite
                && path
                    .file_name()
                    .is_some_and(|name| name == "name" || name == "tags");
            if path.is_symlink() || top_index {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if path.file_name().is_some_and(|name| name == "in.yaml") {
                found.push(path);
            }
        }
    }
    found.sort();
    assert!(
        !found.is_empty(),
        "no inputs under {}: run `git submodule update --init`",
        suite.display()
    );
    found
}

fn read(source: &str) -> Option<Value> {
    let options = serde_saphyr::options! { strict_booleans: true };
    serde_saphyr::from_str_with_options(source, options).ok()
}

/// Every scalar in a value, with its JSON pointer.
fn leaves(value: &Value, pointer: &str, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                leaves(item, &format!("{pointer}/{index}"), out);
            }
        }
        Value::Object(entries) => {
            for (key, item) in entries {
                let key = key.replace('~', "~0").replace('/', "~1");
                leaves(item, &format!("{pointer}/{key}"), out);
            }
        }
        _ => out.push((pointer.to_owned(), value.clone())),
    }
}

fn replace_each_scalar(case: &str, source: &str, document: &Document, tally: &mut Tally) {
    let Some(root) = read(source) else {
        tally.unreadable += 1;
        return;
    };
    let mut scalars = Vec::new();
    leaves(&root, "", &mut scalars);
    for (pointer, scalar) in scalars {
        let Ok(path) = Path::from_pointer(&pointer) else {
            continue;
        };
        let Some(node) = document.node(&path) else {
            continue;
        };
        let owned = node.owned();
        let mut edited = document.clone();
        match edited.replace(&path, &scalar) {
            Ok(()) => {
                let text = edited.as_str();
                assert!(
                    text.starts_with(&source[..owned.start])
                        && text.ends_with(&source[owned.end..]),
                    "{case} {pointer}: bytes outside the node changed:\n{text}"
                );
                assert_eq!(
                    read(text).as_ref(),
                    Some(&root),
                    "{case} {pointer}: the value changed:\n{text}"
                );
                tally.replaced += 1;
            }
            Err(Error::Unsupported { .. } | Error::TagMismatch { .. }) => tally.unsupported += 1,
            Err(error) => panic!("{case} {pointer}: {error}"),
        }
    }
}

#[test]
fn every_scalar_in_the_yaml_test_suite_survives_a_replace_with_itself() {
    let mut tally = Tally::default();
    for input in inputs() {
        tally.inputs += 1;
        let case = input
            .parent()
            .and_then(|dir| dir.strip_prefix(env!("CARGO_MANIFEST_DIR")).ok())
            .map_or_else(
                || input.display().to_string(),
                |dir| dir.display().to_string(),
            );
        let Ok(source) = fs::read_to_string(&input) else {
            tally.unreadable += 1;
            continue;
        };
        match Document::parse(source.as_str()) {
            Ok(document) => replace_each_scalar(&case, &source, &document, &mut tally),
            Err(_) => tally.refused += 1,
        }
    }
    eprintln!("{tally:?}");
    assert!(
        tally.replaced >= REPLACED_AT_PIN,
        "fewer scalars replaced than at the pinned suite commit: {tally:?}"
    );
}

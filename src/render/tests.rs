//! The byte pass against the reader. For each string, every style the pass
//! weighs must hold exactly when the reader reads that style back as the
//! string, in block and in flow context.

use std::path::{Path, PathBuf};
use std::{env, fs};

use granit_parser::{Event, Parser};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};

use super::scan::Scan;
use super::{Context, proven};
use crate::index::Style;

/// Every way the pass and the reader disagree about `value`.
fn disagreements(value: &str) -> Vec<String> {
    let mut found = Vec::new();
    for context in [Context::Block, Context::Flow] {
        let scan = Scan::of(value, context);
        let mut weighed = vec![
            (Style::Plain, scan.plain),
            (Style::SingleQuoted, !scan.double_only()),
            (Style::DoubleQuoted, true),
        ];
        if context == Context::Block {
            weighed.extend([(Style::Literal, scan.literal), (Style::Folded, scan.folded)]);
        }
        for (style, holds) in weighed {
            if holds != proven(value, style, context) {
                found.push(format!(
                    "{style:?} in {context:?}: the pass says {holds} for {value:?}"
                ));
            }
        }
    }
    found
}

/// Fail with every disagreement over `values`, and say how many were seen.
fn assert_agrees<'a>(values: impl IntoIterator<Item = &'a str>, source: &str) {
    let mut seen = 0;
    let mut found = Vec::new();
    for value in values {
        seen += 1;
        found.extend(disagreements(value));
    }
    eprintln!("{source}: {seen} strings");
    assert!(found.is_empty(), "{source}:\n{}", found.join("\n"));
}

/// Every scalar of a YAML text, each line of it, and each word of it.
fn scalars(source: &str, out: &mut Vec<String>) {
    for next in Parser::new_from_str(source) {
        let Ok((event, _)) = next else {
            return;
        };
        if let Event::Scalar(text, ..) = event {
            out.extend(text.lines().map(str::to_owned));
            out.extend(text.split(' ').map(str::to_owned));
            out.push(text.into_owned());
        }
    }
}

/// Every file under `root` that `keep` accepts, links left out.
fn files(root: &Path, keep: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_symlink() {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if keep(&path) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Text the reader keeps as a string when it is written plain.
const PLAIN: &[&str] = &[
    "yes",
    "on",
    "inf",
    "-inf",
    "nan",
    "Infinity",
    "2026-10-15",
    "<<",
    "=",
    "a#b",
    "a:b",
    "-a",
    "?a",
    "a :b",
    "---",
    "...",
    "0x",
    "0o8",
    "1__0",
    "0_0",
    "+",
    "1.5.5",
    "1_000.5",
    "1_0000_0000_0000_0000_0000",
    "0x1_0000_0000_0000_0000",
    ".5e999",
    "a\tb",
    "ünïcode —",
];

/// Text the reader reads as another type, or not at all, when it is
/// written plain.
const QUOTED: &[&str] = &[
    "",
    "true",
    "True",
    "FALSE",
    "null",
    "NULL",
    "~",
    "0",
    "-0",
    "007",
    "0755",
    "0x1F",
    "-0x1f",
    "0o17",
    "0b101",
    "1_000",
    "1e5",
    "1.",
    ".5",
    ".1e309",
    "-.INF",
    ".NaN",
    "+.inf",
    "18446744073709551615",
    "18446744073709551616",
    "-9223372036854775808",
    "-9223372036854775809",
    "1e999",
    "a: b",
    "a:",
    "#a",
    "a #b",
    " a",
    "a ",
    "- a",
    "-",
    "?",
    ":",
    "&a",
    "*a",
    "!a",
    "|",
    ">",
    "'a",
    "\"a",
    "%a",
    "@a",
    "`a",
    "[a",
    "{a",
    ",a",
    "a\nb",
    "\u{1}",
    "a\u{2028}b",
];

#[test]
fn the_pass_holds_what_the_reader_reads() {
    for &value in PLAIN {
        assert!(
            Scan::of(value, Context::Block).plain,
            "{value:?} should be plain"
        );
    }
    for &value in QUOTED {
        assert!(
            !Scan::of(value, Context::Block).plain,
            "{value:?} should be quoted"
        );
    }
    assert!(
        Scan::of("?", Context::Flow).plain,
        "a lone ? is plain in flow"
    );
    for value in ["a,b", "a]", "a{b", "-[", "a:,"] {
        assert!(
            Scan::of(value, Context::Block).plain,
            "{value:?} is plain in a block"
        );
        assert!(
            !Scan::of(value, Context::Flow).plain,
            "{value:?} is quoted in flow"
        );
    }
    assert_agrees(PLAIN.iter().chain(QUOTED).copied(), "table");
}

#[test]
fn every_scalar_of_the_yaml_test_suite_agrees() {
    let suite = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/yaml-test-suite");
    let mut values = Vec::new();
    for file in files(&suite, |path| {
        path.file_name().is_some_and(|name| name == "in.yaml")
    }) {
        scalars(&fs::read_to_string(file).unwrap(), &mut values);
    }
    assert!(!values.is_empty(), "run `git submodule update --init`");
    values.sort();
    values.dedup();
    assert_agrees(values.iter().map(String::as_str), "yaml-test-suite");
}

/// The ledgers under `YAMLED_CORPUS`, such as every consumer's
/// `tasks.yaml`. Some are private, so they are not committed; the build
/// host points the variable at them.
#[test]
fn every_string_of_the_corpus_agrees() {
    let Some(root) = env::var_os("YAMLED_CORPUS") else {
        eprintln!("YAMLED_CORPUS is not set: no corpus checked");
        return;
    };
    let mut values = Vec::new();
    let yaml = |path: &Path| {
        path.extension()
            .is_some_and(|ext| ext == "yaml" || ext == "yml")
    };
    for file in files(Path::new(&root), yaml) {
        scalars(&fs::read_to_string(file).unwrap(), &mut values);
    }
    assert!(!values.is_empty(), "no YAML under YAMLED_CORPUS");
    values.sort();
    values.dedup();
    assert_agrees(values.iter().map(String::as_str), "corpus");
}

/// Pieces that sit on the edges of the rules: indicators, colons, hashes,
/// spaces, numerals and their parts, words the reader resolves, and
/// characters only double quotes carry.
fn hard() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        prop::sample::select(
            &[
                " ", "\t", ":", "#", "-", "?", ",", "[", "]", "{", "}", "'", "\"", "\\", "!", "&",
                "*", "|", ">", "%", "@", "`", "~", ".", "+", "_", "\n", "\r",
            ][..]
        ),
        prop::sample::select(
            &[
                "0", "1", "7", "9", "e", "E", "x", "o", "b", "a", "f", "n", "0x", "0o", "0b",
                "1e5", ".5", "1_0",
            ][..]
        ),
        prop::sample::select(
            &[
                "true",
                "False",
                "null",
                "NULL",
                ".inf",
                ".NaN",
                "inf",
                "nan",
                "yes",
                "---",
                "...",
                "<<",
                "2026-10-15",
                "18446744073709551616",
                "9223372036854775808",
            ][..]
        ),
        prop::sample::select(
            &[
                "é", "—", "\u{85}", "\u{a0}", "\u{2028}", "\u{feff}", "\u{fffe}", "\u{1}",
                "\u{7f}",
            ][..]
        ),
    ];
    prop::collection::vec(piece, 0..8).prop_map(|pieces| pieces.concat())
}

/// Run `strategy` for `cases` values; any disagreement fails.
fn generated(strategy: impl Strategy<Value = String>, cases: u32) {
    TestRunner::new(Config::with_cases(cases))
        .run(&strategy, |value| {
            let found = disagreements(&value);
            if found.is_empty() {
                Ok(())
            } else {
                Err(TestCaseError::fail(found.join("\n")))
            }
        })
        .unwrap();
}

#[test]
fn generated_hard_strings_agree() {
    generated(hard(), 20_000);
}

#[test]
fn generated_characters_agree() {
    let characters = prop::collection::vec(any::<char>(), 0..6);
    generated(characters.prop_map(String::from_iter), 5_000);
}

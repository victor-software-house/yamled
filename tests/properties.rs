//! Random edit sequences on generated ledgers. Each edit either succeeds, and
//! then the document reads back as the model says and every byte outside the
//! edited collection is unchanged, or it is refused and the source is
//! unchanged.

use proptest::prelude::*;
use proptest::test_runner::{TestCaseError, TestRunner};
use serde_json::{Value, json};
use yamled::{Document, Error, Path, Segment};

/// Strings that stress quoting and scalar style.
const WORDS: &[&str] = &[
    "plain",
    "two words",
    "with: colon",
    "# hash",
    "true",
    "null",
    "123",
    "",
    " leading space",
    "it's",
    "say \"hi\"",
    "a, b",
    "[x]",
    "- dash",
    "two\nlines",
    "ünïcode —",
];

#[derive(Clone, Debug)]
struct Row {
    note: String,
    comment: bool,
    blank_before: bool,
}

#[derive(Clone, Debug)]
struct Ledger {
    title: String,
    tags: Vec<String>,
    flow_tags: bool,
    rows: Vec<Row>,
    indent: usize,
}

#[derive(Clone, Debug)]
enum Op {
    ReplaceTitle(String),
    ReplaceTag(usize, String),
    PushTag(String),
    InsertTag(usize, String),
    RemoveTag(usize),
    ReplaceNote(usize, String),
    InsertRow(usize),
    RemoveRow(usize),
    MoveRow(usize, usize),
    ReorderRows(Vec<usize>),
    Reindent(usize),
    ReorderRoot,
}

fn word() -> impl Strategy<Value = String> {
    prop::sample::select(WORDS).prop_map(str::to_owned)
}

fn one_line_word() -> impl Strategy<Value = String> {
    word().prop_filter("one line", |word| !word.contains('\n'))
}

fn ledger() -> impl Strategy<Value = Ledger> {
    let row =
        (word(), any::<bool>(), any::<bool>()).prop_map(|(note, comment, blank_before)| Row {
            note,
            comment,
            blank_before,
        });
    (
        word(),
        prop::collection::vec(one_line_word(), 0..4),
        any::<bool>(),
        prop::collection::vec(row, 0..5),
        prop::sample::select(&[2_usize, 4][..]),
    )
        .prop_map(|(title, tags, flow_tags, rows, indent)| Ledger {
            title,
            tags,
            flow_tags,
            rows,
            indent,
        })
}

fn op() -> impl Strategy<Value = Op> {
    let index = 0..8_usize;
    prop_oneof![
        word().prop_map(Op::ReplaceTitle),
        (index.clone(), one_line_word()).prop_map(|(at, word)| Op::ReplaceTag(at, word)),
        one_line_word().prop_map(Op::PushTag),
        (index.clone(), one_line_word()).prop_map(|(at, word)| Op::InsertTag(at, word)),
        index.clone().prop_map(Op::RemoveTag),
        (index.clone(), word()).prop_map(|(at, word)| Op::ReplaceNote(at, word)),
        index.clone().prop_map(Op::InsertRow),
        index.clone().prop_map(Op::RemoveRow),
        (index.clone(), index.clone()).prop_map(|(from, to)| Op::MoveRow(from, to)),
        prop::collection::vec(any::<usize>(), 8).prop_map(Op::ReorderRows),
        prop::sample::select(&[0_usize, 2, 4][..]).prop_map(Op::Reindent),
        Just(Op::ReorderRoot),
    ]
}

/// A scalar as the generator writes it: plain for lowercase words that read
/// as strings, else a JSON string, which is a valid double-quoted scalar.
fn scalar(value: &str) -> String {
    let plain = !value.is_empty()
        && value.chars().all(|c| c.is_ascii_lowercase() || c == ' ')
        && !value.starts_with(' ')
        && !value.ends_with(' ')
        && !matches!(
            value,
            "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "y" | "n"
        );
    if plain {
        value.to_owned()
    } else {
        Value::String(value.to_owned()).to_string()
    }
}

const LF: char = '\n';

/// Append one line and its break.
fn line(text: &mut String, content: &str) {
    text.push_str(content);
    text.push(LF);
}

fn render(ledger: &Ledger) -> String {
    let mut text = String::new();
    line(&mut text, &format!("title: {}", scalar(&ledger.title)));
    let tags: Vec<String> = ledger.tags.iter().map(|tag| scalar(tag)).collect();
    if tags.is_empty() {
        line(&mut text, "tags: []");
    } else if ledger.flow_tags {
        line(&mut text, &format!("tags: [{}]", tags.join(", ")));
    } else {
        line(&mut text, "tags:");
        for tag in &tags {
            line(&mut text, &format!("  - {tag}"));
        }
    }
    if ledger.rows.is_empty() {
        line(&mut text, "rows: []");
        return text;
    }
    line(&mut text, "rows:");
    let pad = " ".repeat(ledger.indent);
    for (index, row) in ledger.rows.iter().enumerate() {
        if row.blank_before && index > 0 {
            line(&mut text, "");
        }
        if row.comment {
            line(&mut text, &format!("{pad}# about R-{index}"));
        }
        line(&mut text, &format!("{pad}- id: R-{index}"));
        line(&mut text, &format!("{pad}  note: {}", scalar(&row.note)));
    }
    text
}

fn model(ledger: &Ledger) -> Value {
    let rows: Vec<Value> = ledger
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| json!({"id": format!("R-{index}"), "note": row.note}))
        .collect();
    json!({"title": ledger.title, "tags": ledger.tags, "rows": rows})
}

fn read(source: &str) -> Value {
    let options = serde_saphyr::options! { strict_booleans: true };
    serde_saphyr::from_str_with_options(source, options)
        .unwrap_or_else(|error| panic!("the edited text does not read: {error}\n{source}"))
}

fn items(model: &mut Value, key: &str) -> Vec<Value> {
    model[key].as_array().cloned().unwrap_or_default()
}

/// Apply one edit to the document and the model. `Ok(false)` means the edit
/// was refused, which leaves both unchanged.
fn apply(document: &mut Document, model: &mut Value, op: &Op) -> Result<bool, Error> {
    let root = Path::root();
    let tags = root.clone().key("tags");
    let rows = root.clone().key("rows");
    let mut tag_list = items(model, "tags");
    let mut row_list = items(model, "rows");
    match op {
        Op::ReplaceTitle(word) => {
            document.replace(&root.clone().key("title"), word)?;
            model["title"] = json!(word);
        }
        Op::ReplaceTag(at, word) if !tag_list.is_empty() => {
            let at = at % tag_list.len();
            document.replace(&tags.clone().index(at), word)?;
            tag_list[at] = json!(word);
        }
        Op::PushTag(word) => {
            document.push(&tags, word)?;
            tag_list.push(json!(word));
        }
        Op::InsertTag(at, word) => {
            let at = at % (tag_list.len() + 1);
            document.insert_item(&tags, at, word)?;
            tag_list.insert(at, json!(word));
        }
        Op::RemoveTag(at) if !tag_list.is_empty() => {
            let at = at % tag_list.len();
            document.remove(&tags.clone().index(at))?;
            tag_list.remove(at);
        }
        Op::ReplaceNote(at, word) if !row_list.is_empty() => {
            let at = at % row_list.len();
            document.replace(&rows.clone().index(at).key("note"), word)?;
            row_list[at]["note"] = json!(word);
        }
        Op::InsertRow(at) => {
            let at = at % (row_list.len() + 1);
            let row = json!({"id": "R-new", "note": "new"});
            document.insert_item(&rows, at, &row)?;
            row_list.insert(at, row);
        }
        Op::RemoveRow(at) if !row_list.is_empty() => {
            let at = at % row_list.len();
            document.remove(&rows.clone().index(at))?;
            row_list.remove(at);
        }
        Op::MoveRow(from, to) if !row_list.is_empty() => {
            let from = from % row_list.len();
            let to = to % row_list.len();
            let fragment = document.take(&rows.clone().index(from))?;
            document.put(&rows, to, &fragment)?;
            let row = row_list.remove(from);
            row_list.insert(to, row);
        }
        Op::ReorderRows(seed) if row_list.len() > 1 => {
            let mut order: Vec<usize> = (0..row_list.len()).collect();
            order.sort_by_key(|&index| seed[index % seed.len()]);
            let segments: Vec<Segment> = order.iter().map(|&index| Segment::Index(index)).collect();
            document.reorder(&rows, &segments)?;
            let original = row_list.clone();
            for (slot, &index) in order.iter().enumerate() {
                row_list[slot] = original[index].clone();
            }
        }
        Op::Reindent(column) if !row_list.is_empty() => document.reindent(&rows, *column)?,
        Op::ReorderRoot => {
            let order = ["rows", "tags", "title"].map(|key| Segment::Key(key.to_owned()));
            document.reorder(&root, &order)?;
        }
        Op::ReplaceTag(..)
        | Op::RemoveTag(_)
        | Op::ReplaceNote(..)
        | Op::RemoveRow(_)
        | Op::MoveRow(..)
        | Op::ReorderRows(_)
        | Op::Reindent(_) => return Ok(false),
    }
    model["tags"] = Value::Array(tag_list);
    model["rows"] = Value::Array(row_list);
    Ok(true)
}

/// The collection an edit writes into, whose owned lines are the only ones
/// it may change; `None` for an edit of the whole document.
fn scope(op: &Op) -> Option<Path> {
    let root = Path::root();
    match op {
        Op::ReplaceTitle(_) => Some(root.key("title")),
        Op::ReplaceTag(..) | Op::PushTag(_) | Op::InsertTag(..) | Op::RemoveTag(_) => {
            Some(root.key("tags"))
        }
        Op::ReplaceNote(..)
        | Op::InsertRow(_)
        | Op::RemoveRow(_)
        | Op::MoveRow(..)
        | Op::ReorderRows(_)
        | Op::Reindent(_) => Some(root.key("rows")),
        Op::ReorderRoot => None,
    }
}

/// A failed case, with the edit and the text before and after it.
fn failure(op: &Op, why: &str, before: &str, after: &str) -> TestCaseError {
    TestCaseError::fail(format!(
        "{op:?} {why}\n--- before\n{before}--- after\n{after}"
    ))
}

fn check(ledger: &Ledger, ops: &[Op]) -> Result<(), TestCaseError> {
    let source = render(ledger);
    let mut document = Document::parse(source.as_str())
        .unwrap_or_else(|error| panic!("generated ledger does not parse: {error}\n{source}"));
    let mut expected = model(ledger);
    if read(document.as_str()) != expected {
        return Err(TestCaseError::fail(format!(
            "the generated ledger does not read as its model\n{source}"
        )));
    }
    for op in ops {
        let before = document.as_str().to_owned();
        let span = scope(op)
            .and_then(|path| document.node(&path))
            .map(|node| node.owned());
        match apply(&mut document, &mut expected, op) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(
                Error::Unsupported { .. } | Error::WrongKind { .. } | Error::TagMismatch { .. },
            ) => {
                if document.as_str() != before {
                    return Err(failure(
                        op,
                        "was refused but changed the text",
                        &before,
                        document.as_str(),
                    ));
                }
                continue;
            }
            Err(error) => panic!("{op:?} failed: {error}\n{before}"),
        }
        let after = document.as_str();
        if read(after) != expected {
            return Err(failure(
                op,
                "left a value the model does not have",
                &before,
                after,
            ));
        }
        if let Some(span) = span
            && !(after.starts_with(&before[..span.start]) && after.ends_with(&before[span.end..]))
        {
            return Err(failure(
                op,
                "changed bytes outside its collection",
                &before,
                after,
            ));
        }
    }
    Ok(())
}

#[test]
fn random_edits_keep_the_value_and_the_bytes_they_do_not_name() {
    let config = ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    let strategy = (ledger(), prop::collection::vec(op(), 1..8));
    let result = TestRunner::new(config).run(&strategy, |(ledger, ops)| check(&ledger, &ops));
    if let Err(error) = result {
        panic!("{error}");
    }
}

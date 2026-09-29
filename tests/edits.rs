//! Each edit changes only the bytes it names.
#![allow(missing_docs)]

use std::collections::BTreeMap;

use indoc::indoc;
use serde::Serialize;
use yamled::{Document, Error, Path, Position, TextStyle};

const LEDGER: &str = include_str!("fixtures/ledger.yaml");

#[track_caller]
fn doc(source: &str) -> Document {
    Document::parse(source).unwrap_or_else(|error| panic!("fixture does not parse: {error}"))
}

fn root() -> Path {
    Path::root()
}

/// Lines that differ between two texts, found by trimming the common prefix
/// and suffix. Enough to prove an edit touched only the lines it named.
fn changed_lines<'a>(before: &'a str, after: &'a str) -> (Vec<&'a str>, Vec<&'a str>) {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (
        old[prefix..old.len() - suffix].to_vec(),
        new[prefix..new.len() - suffix].to_vec(),
    )
}

#[test]
fn replacing_one_value_leaves_the_rest_of_a_ledger_byte_identical() {
    let mut ledger = Document::parse(LEDGER).unwrap();
    ledger.replace(&root().key("active"), "CTC-008").unwrap();
    let (removed, added) = changed_lines(LEDGER, ledger.as_str());
    assert_eq!(removed, ["active: CTC-006"]);
    assert_eq!(added, ["active: CTC-008"]);
}

#[test]
fn a_plain_title_that_now_needs_quoting_is_quoted_once() {
    let mut document = doc(indoc! {"
        - id: A-1
          title: Old
    "});
    document
        .replace(&root().index(0).key("title"), "A new title: with a colon")
        .unwrap();
    let expected = indoc! {"
        - id: A-1
          title: 'A new title: with a colon'
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_quoted_scalar_stays_quoted_and_a_folded_one_stays_folded() {
    let mut document = doc(indoc! {r#"
        title: "Old"
        outcome: >-
          The old outcome.
        next: kept
    "#});
    document.replace(&root().key("title"), "New").unwrap();
    document
        .replace(&root().key("outcome"), "The new outcome.")
        .unwrap();
    let expected = indoc! {r#"
        title: "New"
        outcome: >-
          The new outcome.
        next: kept
    "#};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_string_that_reads_as_another_type_is_quoted() {
    let mut document = doc(indoc! {"
        flag: x
        count: x
        text: x
    "});
    document.replace(&root().key("flag"), &true).unwrap();
    document.replace(&root().key("count"), &4).unwrap();
    document.replace(&root().key("text"), "true").unwrap();
    let expected = indoc! {"
        flag: true
        count: 4
        text: 'true'
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn replacing_a_list_writes_it_at_the_files_indentation() {
    let mut document = doc(indoc! {"
        row:
          blocked_by: []
          acceptance:
            - Old.
    "});
    document
        .replace(&root().key("row").key("acceptance"), &["First.", "Second."])
        .unwrap();
    let expected = indoc! {"
        row:
          blocked_by: []
          acceptance:
            - First.
            - Second.
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn removing_an_item_keeps_the_next_rows_comment() {
    let mut document = doc(indoc! {"
        queue:
          - id: A-1

          # owns A-2
          - id: A-2
    "});
    document.remove(&root().key("queue").index(0)).unwrap();
    let expected = indoc! {"
        queue:
          # owns A-2
          - id: A-2
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn removing_a_key_takes_its_own_comment_and_nothing_else() {
    let mut document = doc(indoc! {"
        row:
          id: A-1
          # why the outcome
          outcome: gone
          # why the scope
          scope: kept
    "});
    document.remove(&root().key("row").key("outcome")).unwrap();
    let expected = indoc! {"
        row:
          id: A-1
          # why the scope
          scope: kept
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn removing_the_first_key_of_a_row_keeps_the_dash() {
    let mut document = doc(indoc! {"
        - id: A-1
          title: Kept
    "});
    document.remove(&root().index(0).key("id")).unwrap();
    let expected = indoc! {"
        - title: Kept
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_note_is_appended_to_a_block_list() {
    let mut document = doc(indoc! {"
        row:
          notes:
            - first
            - second
    "});
    let notes = root().key("row").key("notes");
    document.push(&notes, "third").unwrap();
    document
        .push_text(&notes, "2026-09-28: a dated note", TextStyle::Folded)
        .unwrap();
    document
        .push(&notes, "2026-09-28: quoted when automatic")
        .unwrap();
    let expected = indoc! {"
        row:
          notes:
            - first
            - second
            - third
            - >-
              2026-09-28: a dated note
            - '2026-09-28: quoted when automatic'
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn folded_text_that_starts_with_a_space_gets_an_indentation_indicator() {
    let mut document = doc(indoc! {"
        notes:
          - first
    "});
    document
        .push_text(&root().key("notes"), " leading space", TextStyle::Folded)
        .unwrap();
    let expected = indoc! {"
        notes:
          - first
          - >2-
             leading space
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn literal_text_keeps_its_line_breaks() {
    let mut document = doc(indoc! {"
        notes:
          - first
    "});
    let text = indoc! {"
        line one

        line three
    "}
    .trim_end();
    document
        .push_text(&root().key("notes"), text, TextStyle::Literal)
        .unwrap();
    let expected = indoc! {"
        notes:
          - first
          - |-
            line one

            line three
    "};
    assert_eq!(document.as_str(), expected);
}

#[derive(Serialize)]
struct Row<'a> {
    id: &'a str,
    title: &'a str,
    acceptance: Vec<&'a str>,
}

#[test]
fn the_first_row_goes_into_an_empty_queue() {
    let mut document = doc(indoc! {"
        # yaml-language-server: $schema=x
        active: null
        queue: []

        archive: []
    "});
    let row = Row {
        id: "T-001",
        title: "First: row",
        acceptance: vec!["It holds."],
    };
    document.push(&root().key("queue"), &row).unwrap();
    let expected = indoc! {r#"
        # yaml-language-server: $schema=x
        active: null
        queue:
          - id: T-001
            title: "First: row"
            acceptance:
              - It holds.

        archive: []
    "#};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_key_is_inserted_at_the_end_before_or_after_a_key() {
    let mut document = doc(indoc! {"
        - id: A-1
          title: Row
    "});
    let row = root().index(0);
    document
        .insert(&row, "scope", "yamled", Position::End)
        .unwrap();
    document
        .insert(&row, "kind", "task", Position::After("id"))
        .unwrap();
    document
        .insert(&row, "rank", &1, Position::Before("id"))
        .unwrap();
    let expected = indoc! {"
        - rank: 1
          id: A-1
          kind: task
          title: Row
          scope: yamled
    "};
    assert_eq!(document.as_str(), expected);
    assert!(matches!(
        document.insert(&row, "id", "again", Position::End),
        Err(Error::KeyExists { .. })
    ));
}

#[test]
fn a_row_moves_from_the_queue_to_the_front_of_the_archive() {
    let mut document = doc(indoc! {"
        queue:
          # owns A-1
          - id: A-1
            title: Moving

          - id: A-2
            title: Staying

        archive:
          - id: A-0
            title: Done
    "});
    let row = document.take(&root().key("queue").index(0)).unwrap();
    document.put(&root().key("archive"), 0, &row).unwrap();
    let expected = indoc! {"
        queue:
          - id: A-2
            title: Staying

        archive:
          # owns A-1
          - id: A-1
            title: Moving
          - id: A-0
            title: Done
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_moved_row_keeps_blank_line_separators() {
    let mut document = doc(indoc! {"
        queue:
          - id: A-1

          - id: A-2

          - id: A-3
    "});
    let row = document.take(&root().key("queue").index(2)).unwrap();
    document.put(&root().key("queue"), 0, &row).unwrap();
    let expected = indoc! {"
        queue:
          - id: A-3

          - id: A-1

          - id: A-2
    "};
    assert_eq!(document.as_str(), expected);
    let row = document.take(&root().key("queue").index(0)).unwrap();
    document.put(&root().key("queue"), 2, &row).unwrap();
    let expected = indoc! {"
        queue:
          - id: A-1

          - id: A-2

          - id: A-3
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn taking_the_only_item_leaves_an_empty_list() {
    let mut document = doc(indoc! {"
        queue:
          - id: A-1
        archive: []
    "});
    let row = document.take(&root().key("queue").index(0)).unwrap();
    let expected = indoc! {"
        - id: A-1
    "};
    assert_eq!(row.as_str(), expected);
    let expected = indoc! {"
        queue: []
        archive: []
    "};
    assert_eq!(document.as_str(), expected);
    document.put(&root().key("archive"), 0, &row).unwrap();
    let expected = indoc! {"
        queue: []
        archive:
          - id: A-1
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_ledger_row_moves_with_only_its_own_lines_changing() {
    let mut ledger = Document::parse(LEDGER).unwrap();
    let before = ledger.node(&root().key("queue").index(0)).unwrap().owned();
    let row_text = LEDGER[before.start..before.end].to_owned();
    let row = ledger.take(&root().key("queue").index(0)).unwrap();
    ledger.put(&root().key("archive"), 0, &row).unwrap();
    let after = ledger
        .node(&root().key("archive").index(0))
        .unwrap()
        .owned();
    assert_eq!(&ledger.as_str()[after.start..after.end], row_text);
    assert_eq!(ledger.node(&root().key("queue")).unwrap().len(), 1);
}

#[test]
fn edits_that_do_not_fit_are_refused_and_change_nothing() {
    let source = indoc! {"
        flow: {a: 1}
        list: [x]
        only:
          key: 1
    "};
    let mut document = Document::parse(source).unwrap();
    assert!(matches!(
        document.replace(&root().key("flow").key("a"), &["b", "c"]),
        Err(Error::Unsupported { .. })
    ));
    assert!(matches!(
        document.remove(&root().key("list").index(0)),
        Err(Error::Unsupported { .. })
    ));
    assert!(matches!(
        document.remove(&root().key("only").key("key")),
        Err(Error::Unsupported { .. })
    ));
    assert!(matches!(
        document.push(&root().key("only"), "x"),
        Err(Error::WrongKind { .. })
    ));
    assert!(matches!(
        document.replace(&root().key("missing"), "x"),
        Err(Error::NoNode { .. })
    ));
    assert_eq!(document.as_str(), source);
}

#[test]
fn a_flow_scalar_with_a_flow_indicator_is_quoted() {
    let mut document = doc(indoc! {"
        list: [x]
        flow: {a: 1}
    "});
    document
        .replace(&root().key("list").index(0), "a, b")
        .unwrap();
    document
        .replace(&root().key("flow").key("a"), "x, y")
        .unwrap();
    let expected = indoc! {"
        list: ['a, b']
        flow: {a: 'x, y'}
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn removing_a_dash_line_key_keeps_the_next_keys_comment() {
    let mut document = doc(indoc! {"
        - id: A-1
          # why the title
          title: Kept
    "});
    document.remove(&root().index(0).key("id")).unwrap();
    let expected = indoc! {"
        # why the title
        - title: Kept
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn emptying_and_refilling_a_list_keeps_the_key_line_comment() {
    let mut document = doc(indoc! {"
        queue: # rows
          - id: A-1
        archive: [] # done
    "});
    let row = document.take(&root().key("queue").index(0)).unwrap();
    document.put(&root().key("archive"), 0, &row).unwrap();
    let expected = indoc! {"
        queue: [] # rows
        archive: # done
          - id: A-1
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_replaced_block_scalar_keeps_its_indentation() {
    let mut document = doc(indoc! {"
        row:
            outcome: >-
                  Old text.
            next: kept
    "});
    document
        .replace(&root().key("row").key("outcome"), "New text.")
        .unwrap();
    let expected = indoc! {"
        row:
            outcome: >-
                  New text.
            next: kept
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_block_scalar_with_an_indicator_is_written_where_the_indicator_says() {
    let mut document = doc(indoc! {"
        row:
            outcome: >-
                  Old text.
    "});
    document
        .replace_text(
            &root().key("row").key("outcome"),
            " leading space",
            TextStyle::Folded,
        )
        .unwrap();
    let expected = indoc! {"
        row:
            outcome: >2-
               leading space
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn emptying_a_list_keeps_its_anchor_before_the_brackets() {
    let mut document = doc(indoc! {"
        queue: &rows # kept
          - id: A-1
        archive: &done []
    "});
    let row = document.take(&root().key("queue").index(0)).unwrap();
    document.put(&root().key("archive"), 0, &row).unwrap();
    let expected = indoc! {"
        queue: &rows [] # kept
        archive: &done
          - id: A-1
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_row_goes_into_a_key_with_no_value() {
    let mut document = doc(indoc! {"
        queue: # empty
        archive: []
    "});
    document.push(&root().key("queue"), "first").unwrap();
    let expected = indoc! {"
        queue: # empty
          - first
        archive: []
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn an_empty_value_on_a_later_line_is_replaced() {
    let mut document = doc(indoc! {"
        queue:
          [] # none yet
        horizon: [
        ]
        archive: []
    "});
    document.push(&root().key("queue"), "first").unwrap();
    document.push(&root().key("horizon"), "later").unwrap();
    let expected = indoc! {"
        queue: # none yet
          - first
        horizon:
          - later
        archive: []
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn an_empty_value_under_comment_lines_is_refused() {
    let source = indoc! {"
        queue:
          # why it is empty
          ~
    "};
    let mut document = Document::parse(source).unwrap();
    assert!(matches!(
        document.push(&root().key("queue"), "first"),
        Err(Error::Unsupported { .. })
    ));
    assert_eq!(document.as_str(), source);
}

#[test]
fn a_replaced_root_collection_starts_at_column_0() {
    let mut document = doc(indoc! {"
        - a
        - b
    "});
    document.replace(&root(), &["c", "d", "e"]).unwrap();
    let expected = indoc! {"
        - c
        - d
        - e
    "};
    assert_eq!(document.as_str(), expected);
    let mut document = doc(indoc! {"
        a: 1
        b: 2
    "});
    let mapping = BTreeMap::from([("left", 1), ("right", 2)]);
    document.replace(&root(), &mapping).unwrap();
    let expected = indoc! {"
        left: 1
        right: 2
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn an_empty_value_on_a_later_line_keeps_its_anchor() {
    let mut document = doc(indoc! {"
        queue:
          &rows [] # none yet
        copy: *rows
    "});
    document.push(&root().key("queue"), "first").unwrap();
    let expected = indoc! {"
        queue: &rows # none yet
          - first
        copy: *rows
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_replaced_root_collection_keeps_its_column() {
    let mut document = doc(indoc! {"
        # indented root
          a: 1
          b: 2
    "});
    let mapping = BTreeMap::from([("left", 1), ("right", 2)]);
    document.replace(&root(), &mapping).unwrap();
    let expected = indoc! {"
        # indented root
          left: 1
          right: 2
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_collection_is_refused_on_the_document_marker_line() {
    let source = indoc! {"
        --- foo
    "};
    let mut document = doc(source);
    let mapping = BTreeMap::from([("left", 1), ("right", 2)]);
    let result = document.replace(&root(), &mapping);
    assert!(matches!(result, Err(Error::Invalid { .. })));
    assert_eq!(document.as_str(), source);
}

#[test]
fn an_empty_value_tagged_as_another_kind_is_not_filled() {
    for source in [
        indoc! {"
            queue: !!null ~
        "},
        indoc! {"
            queue:
              !!null ~
        "},
    ] {
        let mut document = doc(source);
        let refused = document.push(&root().key("queue"), "first");
        assert!(
            matches!(refused, Err(Error::Unsupported { .. })),
            "{source}"
        );
        assert_eq!(document.as_str(), source);
    }
}

#[test]
fn properties_and_comments_from_both_lines_meet_on_the_key_line() {
    let mut document = doc(indoc! {"
        queue: !!seq # a
          &rows [] # b
        copy: *rows
    "});
    document.push(&root().key("queue"), "first").unwrap();
    let expected = indoc! {"
        queue: !!seq &rows # a # b
          - first
        copy: *rows
    "};
    assert_eq!(document.as_str(), expected);
}

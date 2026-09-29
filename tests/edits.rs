//! Each edit changes only the bytes it names.
#![allow(missing_docs)]

use std::collections::BTreeMap;

use indoc::indoc;
use serde::Serialize;
use yamled::{Document, Error, Path, Position, Segment, TextStyle};

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
        only:
          key: 1
    "};
    let mut document = Document::parse(source).unwrap();
    assert!(matches!(
        document.replace(&root().key("flow").key("a"), &["b", "c"]),
        Err(Error::Unsupported { .. })
    ));
    assert!(matches!(
        document.remove(&root().key("flow").key("a")),
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
    let two = BTreeMap::from([("left", 1), ("right", 2)]);
    let one = BTreeMap::from([("left", 1)]);
    let refused = [
        doc(source).replace(&root(), &two),
        doc(source).replace(&root(), &one),
        doc(source).replace(&root(), &["a"]),
    ];
    for result in refused {
        assert!(matches!(result, Err(Error::Invalid { .. })), "{result:?}");
    }
    let mut document = doc(source);
    let _ = document.replace(&root(), &one);
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
fn an_empty_value_with_the_verbatim_sequence_tag_is_filled() {
    let mut document = doc(indoc! {"
        queue: !<tag:yaml.org,2002:seq> []
    "});
    document.push(&root().key("queue"), "first").unwrap();
    let expected = indoc! {"
        queue: !<tag:yaml.org,2002:seq>
          - first
    "};
    assert_eq!(document.as_str(), expected);
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

#[test]
fn a_reorder_that_names_a_child_twice_or_a_missing_one_is_refused() {
    let source = indoc! {"
        a: 1
        b: 2
    "};
    let mut document = doc(source);
    let twice = [Segment::Key("a".to_owned()), Segment::Key("a".to_owned())];
    assert!(matches!(
        document.reorder(&root(), &twice),
        Err(Error::Repeated { .. })
    ));
    let missing = [Segment::Key("c".to_owned()), Segment::Key("a".to_owned())];
    assert!(matches!(
        document.reorder(&root(), &missing),
        Err(Error::NoNode { .. })
    ));
    assert_eq!(document.as_str(), source);
}

#[test]
fn a_reorder_of_keys_on_a_dash_line_is_refused() {
    let source = indoc! {"
        - id: A-1
          title: Row
    "};
    let mut document = doc(source);
    let order = [
        Segment::Key("title".to_owned()),
        Segment::Key("id".to_owned()),
    ];
    assert!(matches!(
        document.reorder(&root().index(0), &order),
        Err(Error::Unsupported { .. })
    ));
    assert_eq!(document.as_str(), source);
}

#[test]
fn a_reorder_keeps_a_file_that_ends_without_a_line_break() {
    let mut document = doc(indoc! {"
        a: 1
        b: 2
    "}
    .trim_end());
    let order = [Segment::Key("b".to_owned()), Segment::Key("a".to_owned())];
    document.reorder(&root(), &order).unwrap();
    let expected = indoc! {"
        b: 2
        a: 1
    "};
    assert_eq!(document.as_str(), expected.trim_end());
}

#[test]
fn flow_items_are_removed_with_one_separator() {
    let removed = |index: usize| {
        let mut document = doc(indoc! {"
            list: [ a, b, c ]
        "});
        document.remove(&root().key("list").index(index)).unwrap();
        document.into_string()
    };
    let lines: Vec<String> = (0..3).map(removed).collect();
    let expected = indoc! {"
        list: [ b, c ]
        list: [ a, c ]
        list: [ a, b ]
    "};
    assert_eq!(lines.concat(), expected);
    let mut document = doc(indoc! {"
        list: [ a ]
    "});
    document.remove(&root().key("list").index(0)).unwrap();
    let expected = indoc! {"
        list: []
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_flow_list_on_several_lines_is_not_edited_item_by_item() {
    let source = indoc! {"
        list: [
          a, b
        ]
    "};
    let mut document = doc(source);
    assert!(matches!(
        document.remove(&root().key("list").index(0)),
        Err(Error::Unsupported { .. })
    ));
    assert_eq!(document.as_str(), source);
}

#[test]
fn a_reindent_that_would_leave_its_key_is_refused() {
    let source = indoc! {"
        outer:
          queue:
            - a
          next: 1
    "};
    let mut document = doc(source);
    assert!(matches!(
        document.reindent(&root().key("outer").key("queue"), 0),
        Err(Error::Invalid { .. })
    ));
    assert_eq!(document.as_str(), source);
}

#[test]
fn a_mapping_is_reindented_to_the_right() {
    let mut document = doc(indoc! {"
        row:
          id: A-1
          notes:
            - Kept.
    "});
    document.reindent(&root().key("row"), 4).unwrap();
    let expected = indoc! {"
        row:
            id: A-1
            notes:
              - Kept.
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn text_is_inserted_at_an_index_in_a_chosen_style() {
    let mut document = doc(indoc! {"
        notes:
          - First.
          - Third.
    "});
    document
        .insert_item_text(&root().key("notes"), 1, "Second: kept", TextStyle::Folded)
        .unwrap();
    let expected = indoc! {"
        notes:
          - First.
          - >-
            Second: kept
          - Third.
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_node_that_keeps_its_trailing_lines_does_not_move_without_them() {
    let source = indoc! {"
        rows:
          - note: |+
              kept

          - id: A-2
    "};
    let rows = root().key("rows");
    let mut document = doc(source);
    assert!(matches!(
        document.take(&rows.clone().index(0)),
        Err(Error::Unsupported { .. })
    ));
    assert!(matches!(
        document.reorder(&rows, &[Segment::Index(1), Segment::Index(0)]),
        Err(Error::Unsupported { .. })
    ));
    assert_eq!(document.as_str(), source);
    document.remove(&rows.index(1)).unwrap();
    let expected = indoc! {"
        rows:
          - note: |+
              kept

    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_kept_scalar_whose_text_starts_with_blank_lines_does_not_move() {
    let source = indoc! {"
        rows:
          - note: |+

              kept

          - id: A-2
    "};
    let mut document = doc(source);
    assert!(matches!(
        document.take(&root().key("rows").index(0)),
        Err(Error::Unsupported { .. })
    ));
    assert_eq!(document.as_str(), source);
}

#[test]
fn a_replaced_value_keeps_its_anchor_and_the_key_line_comment() {
    let mut document = doc(indoc! {"
        key: &shared old # note
        copy: *shared
        list: # rows
          - a
    "});
    document.replace(&root().key("key"), "new").unwrap();
    document.replace(&root().key("list"), "none").unwrap();
    let expected = indoc! {"
        key: &shared new # note
        copy: *shared
        list: none # rows
    "};
    assert_eq!(document.as_str(), expected);
    document.replace(&root().key("key"), &["x", "z"]).unwrap();
    let expected = indoc! {"
        key: &shared # note
          - x
          - z
        copy: *shared
        list: none # rows
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_row_goes_into_a_list_written_at_its_keys_column() {
    let mut document = doc(indoc! {"
        rows:
        - id: A-1
          note: plain
    "});
    let row = BTreeMap::from([("id", "A-0"), ("note", "new")]);
    document.insert_item(&root().key("rows"), 0, &row).unwrap();
    let expected = indoc! {"
        rows:
        - id: A-0
          note: new
        - id: A-1
          note: plain
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_block_scalar_item_or_root_is_replaced_with_its_header() {
    let mut document = doc(indoc! {"
        - |
          old text
        - kept
    "});
    document
        .replace_text(&root().index(0), "new text", TextStyle::Folded)
        .unwrap();
    let expected = indoc! {"
        - >-
          new text
        - kept
    "};
    assert_eq!(document.as_str(), expected);
    let mut document = doc(indoc! {"
        --- >
          folded
    "});
    document.replace(&root(), "kept folded").unwrap();
    let expected = indoc! {"
        --- >-
              kept folded
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn an_empty_value_is_written_after_its_colon() {
    let mut document = doc(indoc! {"
        a:
        b: 2
    "});
    document.replace(&root().key("a"), "one").unwrap();
    let expected = indoc! {"
        a: one
        b: 2
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_kept_scalar_is_replaced_with_the_blank_lines_it_owns() {
    let mut document = doc(indoc! {"
        keep: |+
          text

        next: 1
    "});
    document
        .replace_text(&root().key("keep"), "new\n\n", TextStyle::Auto)
        .unwrap();
    let expected = indoc! {"
        keep: |+
          new

        next: 1
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn an_empty_block_scalar_is_replaced_without_touching_the_next_key() {
    let mut document = doc(indoc! {"
        clip: >

        next: 1
    "});
    document.replace(&root().key("clip"), "text").unwrap();
    let expected = indoc! {"
        clip: >-
          text

        next: 1
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn an_empty_tagged_item_gets_its_value_after_a_space() {
    let mut document = doc(indoc! {"
        - !!str
        - b
    "});
    document.replace(&root().index(0), "a").unwrap();
    let expected = indoc! {"
        - !!str a
        - b
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn an_explicit_key_with_a_comment_finds_its_colon_on_the_next_line() {
    let mut document = doc(indoc! {"
        ? a # c
        : b
    "});
    document.replace(&root().key("a"), "z").unwrap();
    let expected = indoc! {"
        ? a # c
        : z
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_block_scalar_replaced_by_one_line_keeps_its_header_comment() {
    let mut document = doc(indoc! {"
        key: | # note
          old
        next: 1
    "});
    document
        .replace_text(&root().key("key"), "new", TextStyle::Auto)
        .unwrap();
    let expected = indoc! {"
        key: |- # note
          new
        next: 1
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_comment_at_the_start_of_a_line_hides_its_colon() {
    let mut document = doc(indoc! {"
        ? a
        # x: y
        : b
    "});
    document.replace(&root().key("a"), "z").unwrap();
    let expected = indoc! {"
        ? a
        # x: y
        : z
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_tag_stays_only_in_front_of_a_value_it_fits() {
    let source = indoc! {"
        port: !!int 8080
        name: !!str old
        ports: !!seq
          - 1
    "};
    let mut document = doc(source);
    assert!(matches!(
        document.replace(&root().key("port"), "abc"),
        Err(Error::Unsupported { .. })
    ));
    assert!(matches!(
        document.replace(&root().key("name"), &["a", "b"]),
        Err(Error::Unsupported { .. })
    ));
    assert_eq!(document.as_str(), source);
    document.replace(&root().key("port"), &8081).unwrap();
    document.replace(&root().key("name"), "new").unwrap();
    document.replace(&root().key("ports"), &[2, 3]).unwrap();
    let expected = indoc! {"
        port: !!int 8081
        name: !!str new
        ports: !!seq
          - 2
          - 3
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_tag_in_a_flow_collection_or_at_the_root_must_fit_too() {
    let source = indoc! {"
        ports: [!!int 1, !t, 2]
        named: {a: !!int 1}
    "};
    let mut document = doc(source);
    for path in [root().key("ports").index(0), root().key("named").key("a")] {
        assert!(matches!(
            document.replace(&path, "abc"),
            Err(Error::Unsupported { .. })
        ));
    }
    assert_eq!(document.as_str(), source);
    document.replace(&root().key("ports").index(0), &5).unwrap();
    document
        .replace(&root().key("ports").index(2), "abc")
        .unwrap();
    document.replace(&root().key("named").key("a"), &6).unwrap();
    let expected = indoc! {"
        ports: [!!int 5, !t, abc]
        named: {a: !!int 6}
    "};
    assert_eq!(document.as_str(), expected);

    let source = indoc! {"
        --- !!int 5
    "};
    let mut document = doc(source);
    assert!(matches!(
        document.replace(&root(), "abc"),
        Err(Error::Unsupported { .. })
    ));
    document.replace(&root(), &6).unwrap();
    assert_eq!(
        document.as_str(),
        indoc! {"
        --- !!int 6
    "}
    );
}

#[test]
fn a_redefined_secondary_handle_is_not_a_core_tag() {
    let source = indoc! {"
        %TAG !! tag:example.com,2000:app/
        ---
        port: !!int 1 - 3
    "};
    let mut document = doc(source);
    document.replace(&root().key("port"), "4 - 6").unwrap();
    let expected = indoc! {"
        %TAG !! tag:example.com,2000:app/
        ---
        port: !!int 4 - 6
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_bare_tag_counts_as_a_string_and_a_dash_comment_is_not_a_tag() {
    let source = indoc! {"
        key: ! 123
        list:
          - # !!str note
            5
    "};
    let mut document = doc(source);
    assert!(matches!(
        document.replace(&root().key("key"), &8081),
        Err(Error::Unsupported { .. })
    ));
    assert_eq!(document.as_str(), source);
    document.replace(&root().key("key"), &["a"]).unwrap();
    document.replace(&root().key("key"), "text").unwrap();
    document.replace(&root().key("list").index(0), &6).unwrap();
    let back = document.node(&root().key("list").index(0)).unwrap();
    assert_eq!(back.text(), "6");
}

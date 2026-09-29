//! Edit sequences a work-queue tool runs on its ledger: add, start, archive,
//! park, promote, and reorder rows. Each proves the file keeps every byte the
//! sequence does not name.
#![allow(missing_docs)]

use indoc::{formatdoc, indoc};
use serde::Serialize;
use yamled::{Document, Path, Position, Spacing};

#[track_caller]
fn doc(source: &str) -> Document {
    Document::parse(source).unwrap_or_else(|error| panic!("fixture does not parse: {error}"))
}

fn root() -> Path {
    Path::root()
}

fn queue() -> Path {
    root().key("queue")
}

#[derive(Serialize)]
struct Row<'a> {
    id: &'a str,
    title: &'a str,
    blocked_by: Vec<&'a str>,
    acceptance: Vec<&'a str>,
}

const NO_ITEMS: [&str; 0] = [];

#[test]
fn new_rows_keep_the_spacing_and_comments_of_their_neighbours() {
    let mut document = doc(indoc! {r#"
        queue:
          - id: A-1
            title: "Quoted: a colon"
            outcome: >-
              Folded text
              on two lines.

          # owns A-2
          - id: A-2
            scope: release   # beside a key
            blocked_by: [A-1]
        archive: []
    "#});
    let third = Row {
        id: "A-3",
        title: "Third",
        blocked_by: vec!["A-2"],
        acceptance: vec!["It holds too."],
    };
    document.push(&queue(), &third).unwrap();
    let middle = Row {
        id: "A-4",
        title: "Middle",
        blocked_by: Vec::new(),
        acceptance: vec!["It holds."],
    };
    document.push(&queue(), &middle).unwrap();
    let row = document.take(&queue().index(3)).unwrap();
    document.put(&queue(), 1, &row).unwrap();
    let expected = indoc! {r#"
        queue:
          - id: A-1
            title: "Quoted: a colon"
            outcome: >-
              Folded text
              on two lines.

          - id: A-4
            title: Middle
            blocked_by: []
            acceptance:
              - It holds.

          # owns A-2
          - id: A-2
            scope: release   # beside a key
            blocked_by: [A-1]

          - id: A-3
            title: Third
            blocked_by:
              - A-2
            acceptance:
              - It holds too.
        archive: []
    "#};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_list_too_short_to_show_its_spacing_uses_the_configured_one() {
    let source = indoc! {"
        queue:
          - id: A-1

          # owns A-2
          - id: A-2
        archive: []
    "};
    let to_front = |spacing| {
        let mut document = doc(source).with_spacing(spacing);
        let row = document.take(&queue().index(1)).unwrap();
        document.put(&queue(), 0, &row).unwrap();
        document.into_string()
    };
    let expected = indoc! {"
        queue:
          # owns A-2
          - id: A-2

          - id: A-1
        archive: []
    "};
    assert_eq!(to_front(Spacing::Blank), expected);
    let expected = indoc! {"
        queue:
          # owns A-2
          - id: A-2
          - id: A-1
        archive: []
    "};
    assert_eq!(to_front(Spacing::Tight), expected);
}

#[test]
fn a_list_that_shows_its_spacing_keeps_it_whatever_is_configured() {
    let mut document = doc(indoc! {"
        queue:
          - id: A-1
          - id: A-2
          - id: A-3
    "})
    .with_spacing(Spacing::Blank);
    let row = document.take(&queue().index(2)).unwrap();
    document.put(&queue(), 1, &row).unwrap();
    let expected = indoc! {"
        queue:
          - id: A-1
          - id: A-3
          - id: A-2
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_list_with_both_kinds_of_gap_uses_the_configured_spacing() {
    let source = indoc! {"
        tags:
          - a
          - b

          - c
    "};
    let pushed = |spacing| {
        let mut document = doc(source).with_spacing(spacing);
        document.push(&root().key("tags"), "d").unwrap();
        document.into_string()
    };
    let expected = indoc! {"
        tags:
          - a
          - b

          - c

          - d
    "};
    assert_eq!(pushed(Spacing::Blank), expected);
    let expected = indoc! {"
        tags:
          - a
          - b

          - c
          - d
    "};
    assert_eq!(pushed(Spacing::Tight), expected);
}

#[test]
fn an_archived_row_is_reshaped_in_its_new_place() {
    let mut document = doc(indoc! {r#"
        active: A-1

        queue:
          - id: A-1
            title: "Q: a colon"
            outcome: >-
              Folded text
              on two lines.
            blocked_by: []
            acceptance:
              - It holds.

          # owns A-2
          - id: A-2
            scope: release   # beside a key
            blocked_by: [A-1]
        archive: []
    "#});
    let row = document.take(&queue().index(0)).unwrap();
    let archived = root().key("archive").index(0);
    document.put(&root().key("archive"), 0, &row).unwrap();
    document
        .remove(&archived.clone().key("blocked_by"))
        .unwrap();
    document
        .remove(&archived.clone().key("acceptance"))
        .unwrap();
    document
        .insert(&archived, "completed", "2026-01-01T00:00:00", Position::End)
        .unwrap();
    document
        .insert(&archived, "evidence", &["It shipped."], Position::End)
        .unwrap();
    document
        .insert(&archived, "disposition", "completed", Position::End)
        .unwrap();
    document
        .replace(&queue().index(0).key("blocked_by"), &NO_ITEMS)
        .unwrap();
    document.replace(&root().key("active"), "A-2").unwrap();
    let expected = indoc! {r#"
        active: A-2

        queue:
          # owns A-2
          - id: A-2
            scope: release   # beside a key
            blocked_by: []
        archive:
          - id: A-1
            title: "Q: a colon"
            outcome: >-
              Folded text
              on two lines.
            completed: 2026-01-01T00:00:00
            evidence:
              - It shipped.
            disposition: completed
    "#};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_middle_row_leaves_one_blank_line_and_a_blocker_leaves_its_list() {
    let mut document = doc(indoc! {"
        queue:
          - id: A-1
            acceptance: [It holds.]

          - id: A-2
            acceptance: [It also holds.]

          - id: A-3
            blocked_by:
              - A-1
              - A-2
            acceptance: [It holds too.]
    "});
    let row = document.take(&queue().index(1)).unwrap();
    let expected = indoc! {"
        - id: A-2
          acceptance: [It also holds.]
    "};
    assert_eq!(row.as_str(), expected);
    document
        .remove(&queue().index(1).key("blocked_by").index(0))
        .unwrap();
    let expected = indoc! {"
        queue:
          - id: A-1
            acceptance: [It holds.]

          - id: A-3
            blocked_by:
              - A-2
            acceptance: [It holds too.]
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_blank_line_under_the_key_stays_when_the_first_row_goes() {
    let mut document = doc(indoc! {"
        active: A-1
        queue:

          - id: A-1

          - id: A-2
        archive: []
    "});
    document.take(&queue().index(0)).unwrap();
    let expected = indoc! {"
        active: A-1
        queue:

          - id: A-2
        archive: []
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn archiving_the_only_row_empties_the_queue_and_clears_the_pointer() {
    let mut document = doc(indoc! {"
        active: A-1

        queue:
          - id: A-1
            title: The only row
        archive: []
        horizon: []
    "});
    let row = document.take(&queue().index(0)).unwrap();
    document.put(&root().key("archive"), 0, &row).unwrap();
    document
        .replace(&root().key("active"), &None::<&str>)
        .unwrap();
    let expected = indoc! {"
        active: null

        queue: []
        archive:
          - id: A-1
            title: The only row
        horizon: []
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_parked_row_goes_into_a_section_added_at_the_end() {
    let mut document = doc(indoc! {"
        active: null
        queue:
          - id: A-1
            outcome: The question has an answer.
            blocked_by: []
            acceptance:
              - It holds.
        archive: []
    "});
    let horizon = root().key("horizon");
    let parked = horizon.clone().index(0);
    document
        .insert(&root(), "horizon", &NO_ITEMS, Position::End)
        .unwrap();
    let row = document.take(&queue().index(0)).unwrap();
    document.put(&horizon, 0, &row).unwrap();
    document.remove(&parked.clone().key("blocked_by")).unwrap();
    document.remove(&parked.clone().key("acceptance")).unwrap();
    document
        .insert(&parked, "kind", "research", Position::End)
        .unwrap();
    document
        .insert(&parked, "open", "The fact is missing.", Position::End)
        .unwrap();
    let expected = indoc! {"
        active: null
        queue: []
        archive: []
        horizon:
          - id: A-1
            outcome: The question has an answer.
            kind: research
            open: The fact is missing.
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_promoted_row_carries_its_comment_across_sections() {
    let mut document = doc(indoc! {"
        active: A-1

        queue:
          - id: A-1
            acceptance:
              - It holds.
        archive: []
        horizon:
          # owns A-2
          - id: A-2
            kind: research
            open: The fact was missing.
            notes:
              - Keep this.
    "})
    .with_spacing(Spacing::Blank);
    let promoted = queue().index(1);
    let row = document.take(&root().key("horizon").index(0)).unwrap();
    document.put(&queue(), 1, &row).unwrap();
    document.remove(&promoted.clone().key("kind")).unwrap();
    document.remove(&promoted.clone().key("open")).unwrap();
    document
        .insert(&promoted, "blocked_by", &["A-1"], Position::End)
        .unwrap();
    document
        .insert(&promoted, "acceptance", &["It holds."], Position::End)
        .unwrap();
    let expected = indoc! {"
        active: A-1

        queue:
          - id: A-1
            acceptance:
              - It holds.

          # owns A-2
          - id: A-2
            notes:
              - Keep this.
            blocked_by:
              - A-1
            acceptance:
              - It holds.
        archive: []
        horizon: []
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_ledger_prints_back_exactly_as_it_was_read() {
    let tidy = indoc! {"
        style:
          section_order: [queue, horizon, archive]

        active: A-2
        queue:
          - id: A-2
            outcome: >-
              Folded text
              on two lines.
            acceptance: [It holds.]

        horizon:
          - id: A-3

        archive:
          # owns A-1
          - id: A-1
    "};
    let loose_comment = indoc! {"
        active: null
        queue: []

        # A note with blank lines around it, so it belongs to no list.

        archive: []
    "};
    let untidy = formatdoc! {"
        prefix: A{trailing}
        active: A-2



        queue:

          - id: A-2
            title: Row{trailing}
        horizon: []


    ",
        trailing = "   ",
    };
    let unterminated = tidy.trim_end();
    for source in [tidy, loose_comment, &untidy, unterminated] {
        assert_eq!(doc(source).as_str(), source);
    }
    for source in [tidy, &untidy, unterminated] {
        let mut document = doc(source);
        document.replace(&root().key("active"), "A-2").unwrap();
        assert_eq!(document.as_str(), source);
    }
}

#[test]
fn a_replaced_value_keeps_the_spaces_after_it() {
    let mut document = doc(&formatdoc! {"
        title: Row{trailing}
        next: 1
    ",
        trailing = "   ",
    });
    document.replace(&root().key("title"), "New").unwrap();
    let expected = formatdoc! {"
        title: New{trailing}
        next: 1
    ",
        trailing = "   ",
    };
    assert_eq!(document.as_str(), expected);
}

#[test]
fn sorting_rows_moves_each_with_its_comment() {
    let mut document = doc(indoc! {"
        archive:
          - id: A-1
            completed: 2026-08-14T09:00:00

          # owns A-3
          - id: A-3
            completed: 2026-08-16T23:08:28

          - id: A-2
            completed: 2026-08-15T12:30:00
        horizon: []
    "});
    let archive = root().key("archive");
    let row = document.take(&archive.clone().index(0)).unwrap();
    document.put(&archive, 2, &row).unwrap();
    let expected = indoc! {"
        archive:
          # owns A-3
          - id: A-3
            completed: 2026-08-16T23:08:28

          - id: A-2
            completed: 2026-08-15T12:30:00

          - id: A-1
            completed: 2026-08-14T09:00:00
        horizon: []
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_literal_note_becomes_a_list_of_paragraphs() {
    let mut document = doc(indoc! {"
        schema_version: 3
        queue:
          - id: A-1
            notes: |-
              First paragraph.

              Second paragraph.
        archive: []
    "});
    document.replace(&root().key("schema_version"), &4).unwrap();
    document
        .replace(
            &queue().index(0).key("notes"),
            &["First paragraph.", "Second paragraph."],
        )
        .unwrap();
    let expected = indoc! {"
        schema_version: 4
        queue:
          - id: A-1
            notes:
              - First paragraph.
              - Second paragraph.
        archive: []
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_row_moved_to_the_front_is_byte_identical() {
    let mut document = doc(indoc! {r#"
        active: A-2

        queue:
          - id: A-2
            outcome: Plain.

          - id: A-3
            outcome: Plain.

          - id: A-9
            acceptance:
              - A long line — with a dash and `code` that stays on one line however long it runs past any wrap width.
              - "`x` is quoted: with a colon"
            notes:
              - >-
                Folded note that
                wraps here.

        horizon:
          - id: A-7
    "#});
    let row = document.take(&queue().index(2)).unwrap();
    let expected = indoc! {r#"
        - id: A-9
          acceptance:
            - A long line — with a dash and `code` that stays on one line however long it runs past any wrap width.
            - "`x` is quoted: with a colon"
          notes:
            - >-
              Folded note that
              wraps here.
    "#};
    assert_eq!(row.as_str(), expected);
    document.put(&queue(), 0, &row).unwrap();
    document.replace(&root().key("active"), "A-9").unwrap();
    let expected = indoc! {r#"
        active: A-9

        queue:
          - id: A-9
            acceptance:
              - A long line — with a dash and `code` that stays on one line however long it runs past any wrap width.
              - "`x` is quoted: with a colon"
            notes:
              - >-
                Folded note that
                wraps here.

          - id: A-2
            outcome: Plain.

          - id: A-3
            outcome: Plain.

        horizon:
          - id: A-7
    "#};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn an_empty_section_moves_above_the_comment_the_next_key_owns() {
    let mut document = doc(indoc! {"
        queue: []

        # Everything below this line is history, not work.
        archive:
          - id: A-1
        horizon: []
    "});
    document.remove(&root().key("horizon")).unwrap();
    document
        .insert(&root(), "horizon", &NO_ITEMS, Position::Before("archive"))
        .unwrap();
    let expected = indoc! {"
        queue: []

        horizon: []
        # Everything below this line is history, not work.
        archive:
          - id: A-1
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_loose_comment_stays_above_a_key_inserted_below_it() {
    let mut document = doc(indoc! {"
        queue: []

        # A loose note.

        archive: []
    "});
    document
        .insert(&root(), "horizon", &NO_ITEMS, Position::Before("archive"))
        .unwrap();
    let expected = indoc! {"
        queue: []

        # A loose note.

        horizon: []
        archive: []
    "};
    assert_eq!(document.as_str(), expected);
}

#[test]
fn a_row_moved_into_an_empty_list_is_indented_to_it() {
    let mut document = doc(indoc! {"
        queue:
            # owns A-1
            - id: A-1
              outcome: >-
                Folded text
                on two lines.
              acceptance:
                - It holds.
        archive: []
    "});
    let row = document.take(&queue().index(0)).unwrap();
    document.put(&queue(), 0, &row).unwrap();
    let expected = indoc! {"
        queue:
          # owns A-1
          - id: A-1
            outcome: >-
              Folded text
              on two lines.
            acceptance:
              - It holds.
        archive: []
    "};
    assert_eq!(document.as_str(), expected);
}

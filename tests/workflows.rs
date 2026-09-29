//! Edit sequences a work-queue tool runs on its ledger: add, start, archive,
//! park, promote, and reorder rows.
//!
//! Each edit is checked twice. The value at the edited path is read back
//! through `serde-saphyr` and compared with what was written, and the whole
//! file is recorded as a snapshot with every line marked removed (`-`),
//! added (`+`), or kept (` `), so the snapshot is both the result and the
//! diff that proves nothing else moved.
#![allow(missing_docs)]

use indoc::{formatdoc, indoc};
use serde::Serialize;
use serde_json::Value;
use similar::{ChangeTag, TextDiff};
use yamled::{Document, Fragment, Path, Position, Segment, Spacing};

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

/// A document under test, with the text it started from.
struct Ledger {
    before: String,
    document: Document,
}

/// A taken item and the value it held.
struct Moved {
    fragment: Fragment,
    value: Value,
}

#[track_caller]
fn json<T: Serialize + ?Sized>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|error| panic!("not a JSON value: {error}"))
}

/// `value` without the child `segment` names.
#[track_caller]
fn without(mut value: Value, segment: &Segment) -> Value {
    match (&mut value, segment) {
        (Value::Array(items), Segment::Index(index)) => {
            items.remove(*index);
        }
        (Value::Object(entries), Segment::Key(key)) => {
            entries.remove(key);
        }
        _ => panic!("{segment:?} does not name a child of {value}"),
    }
    value
}

impl Ledger {
    #[track_caller]
    fn new(source: &str) -> Self {
        let document = Document::parse(source)
            .unwrap_or_else(|error| panic!("fixture does not parse: {error}"));
        Self {
            before: source.to_owned(),
            document,
        }
    }

    fn spaced(mut self, spacing: Spacing) -> Self {
        self.document = self.document.with_spacing(spacing);
        self
    }

    /// The value at `path`, read from the current text by the reader
    /// settings yamled writes for.
    #[track_caller]
    fn value(&self, path: &Path) -> Option<Value> {
        let options = serde_saphyr::options! { strict_booleans: true };
        let root: Value = serde_saphyr::from_str_with_options(self.document.as_str(), options)
            .unwrap_or_else(|error| panic!("the edited text does not read: {error}"));
        root.pointer(&path.to_string()).cloned()
    }

    #[track_caller]
    fn parent_of(path: &Path) -> (Path, &Segment) {
        match (path.parent(), path.segments().last()) {
            (Some(parent), Some(last)) => (parent, last),
            _ => panic!("{path} has no parent"),
        }
    }

    #[track_caller]
    fn replace<T: Serialize + ?Sized>(&mut self, path: &Path, value: &T) {
        self.document
            .replace(path, value)
            .unwrap_or_else(|error| panic!("replace {path}: {error}"));
        assert_eq!(self.value(path), Some(json(value)), "{path} after replace");
    }

    #[track_caller]
    fn insert<T: Serialize + ?Sized>(
        &mut self,
        path: &Path,
        key: &str,
        value: &T,
        position: Position<'_>,
    ) {
        self.document
            .insert(path, key, value, position)
            .unwrap_or_else(|error| panic!("insert {key} into {path}: {error}"));
        let at = path.clone().key(key);
        assert_eq!(self.value(&at), Some(json(value)), "{at} after insert");
    }

    #[track_caller]
    fn push<T: Serialize + ?Sized>(&mut self, path: &Path, value: &T) {
        let length = self.document.node(path).map_or(0, |node| node.len());
        self.document
            .push(path, value)
            .unwrap_or_else(|error| panic!("push onto {path}: {error}"));
        let at = path.clone().index(length);
        assert_eq!(self.value(&at), Some(json(value)), "{at} after push");
    }

    #[track_caller]
    fn remove(&mut self, path: &Path) {
        let (parent, last) = Self::parent_of(path);
        let expected = self.value(&parent).map(|value| without(value, last));
        self.document
            .remove(path)
            .unwrap_or_else(|error| panic!("remove {path}: {error}"));
        assert_eq!(self.value(&parent), expected, "{parent} after remove");
    }

    #[track_caller]
    fn take(&mut self, path: &Path) -> Moved {
        let (parent, last) = Self::parent_of(path);
        let value = self
            .value(path)
            .unwrap_or_else(|| panic!("nothing at {path}"));
        let expected = self.value(&parent).map(|value| without(value, last));
        let fragment = self
            .document
            .take(path)
            .unwrap_or_else(|error| panic!("take {path}: {error}"));
        assert_eq!(self.value(&parent), expected, "{parent} after take");
        Moved { fragment, value }
    }

    #[track_caller]
    fn put(&mut self, path: &Path, index: usize, moved: &Moved) {
        self.document
            .put(path, index, &moved.fragment)
            .unwrap_or_else(|error| panic!("put into {path}: {error}"));
        let at = path.clone().index(index);
        assert_eq!(self.value(&at), Some(moved.value.clone()), "{at} after put");
    }

    /// The whole file, each line marked with what happened to it, under a
    /// count of the lines removed and added.
    fn marked(&self) -> String {
        let diff = TextDiff::from_lines(&self.before, self.document.as_str());
        let (mut removed, mut added) = (0, 0);
        let mut body = String::new();
        for change in diff.iter_all_changes() {
            let mark = match change.tag() {
                ChangeTag::Delete => {
                    removed += 1;
                    '-'
                }
                ChangeTag::Insert => {
                    added += 1;
                    '+'
                }
                ChangeTag::Equal => ' ',
            };
            body.push(mark);
            body.push_str(change.value());
        }
        format!("# -{removed} +{added}\n{body}")
    }
}

#[test]
fn new_rows_keep_the_spacing_and_comments_of_their_neighbours() {
    let mut ledger = Ledger::new(indoc! {r#"
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
    ledger.push(&queue(), &third);
    let middle = Row {
        id: "A-4",
        title: "Middle",
        blocked_by: Vec::new(),
        acceptance: vec!["It holds."],
    };
    ledger.push(&queue(), &middle);
    let row = ledger.take(&queue().index(3));
    ledger.put(&queue(), 1, &row);
    insta::assert_snapshot!(ledger.marked(), @r#"
    # -0 +13
     queue:
       - id: A-1
         title: "Quoted: a colon"
         outcome: >-
           Folded text
           on two lines.
     
    +  - id: A-4
    +    title: Middle
    +    blocked_by: []
    +    acceptance:
    +      - It holds.
    +
       # owns A-2
       - id: A-2
         scope: release   # beside a key
         blocked_by: [A-1]
    +
    +  - id: A-3
    +    title: Third
    +    blocked_by:
    +      - A-2
    +    acceptance:
    +      - It holds too.
     archive: []
    "#);
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
        let mut ledger = Ledger::new(source).spaced(spacing);
        let row = ledger.take(&queue().index(1));
        ledger.put(&queue(), 0, &row);
        ledger
    };
    let blank = to_front(Spacing::Blank);
    insta::assert_snapshot!(blank.marked(), @"
    # -2 +2
     queue:
    -  - id: A-1
    -
       # owns A-2
       - id: A-2
    +
    +  - id: A-1
     archive: []
    ");
    let tight = to_front(Spacing::Tight);
    insta::assert_snapshot!(tight.marked(), @"
    # -2 +1
     queue:
    -  - id: A-1
    -
       # owns A-2
       - id: A-2
    +  - id: A-1
     archive: []
    ");
}

#[test]
fn a_list_that_shows_its_spacing_keeps_it_whatever_is_configured() {
    let mut ledger = Ledger::new(indoc! {"
        queue:
          - id: A-1
          - id: A-2
          - id: A-3
    "})
    .spaced(Spacing::Blank);
    let row = ledger.take(&queue().index(2));
    ledger.put(&queue(), 1, &row);
    insta::assert_snapshot!(ledger.marked(), @"
    # -1 +1
     queue:
       - id: A-1
    +  - id: A-3
       - id: A-2
    -  - id: A-3
    ");
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
        let mut ledger = Ledger::new(source).spaced(spacing);
        ledger.push(&root().key("tags"), "d");
        ledger
    };
    let blank = pushed(Spacing::Blank);
    insta::assert_snapshot!(blank.marked(), @"
    # -0 +2
     tags:
       - a
       - b
     
       - c
    +
    +  - d
    ");
    let tight = pushed(Spacing::Tight);
    insta::assert_snapshot!(tight.marked(), @"
    # -0 +1
     tags:
       - a
       - b
     
       - c
    +  - d
    ");
}

#[test]
fn an_archived_row_is_reshaped_in_its_new_place() {
    let mut ledger = Ledger::new(indoc! {r#"
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
    let row = ledger.take(&queue().index(0));
    let archived = root().key("archive").index(0);
    ledger.put(&root().key("archive"), 0, &row);
    ledger.remove(&archived.clone().key("blocked_by"));
    ledger.remove(&archived.clone().key("acceptance"));
    ledger.insert(&archived, "completed", "2026-01-01T00:00:00", Position::End);
    ledger.insert(&archived, "evidence", &["It shipped."], Position::End);
    ledger.insert(&archived, "disposition", "completed", Position::End);
    ledger.replace(&queue().index(0).key("blocked_by"), &NO_ITEMS);
    ledger.replace(&root().key("active"), "A-2");
    insta::assert_snapshot!(ledger.marked(), @r#"
    # -10 +10
    -active: A-1
    +active: A-2
     
     queue:
    +  # owns A-2
    +  - id: A-2
    +    scope: release   # beside a key
    +    blocked_by: []
    +archive:
       - id: A-1
         title: "Q: a colon"
         outcome: >-
           Folded text
           on two lines.
    -    blocked_by: []
    -    acceptance:
    -      - It holds.
    -
    -  # owns A-2
    -  - id: A-2
    -    scope: release   # beside a key
    -    blocked_by: [A-1]
    -archive: []
    +    completed: 2026-01-01T00:00:00
    +    evidence:
    +      - It shipped.
    +    disposition: completed
    "#);
}

#[test]
fn a_middle_row_leaves_one_blank_line_and_a_blocker_leaves_its_list() {
    let mut ledger = Ledger::new(indoc! {"
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
    let row = ledger.take(&queue().index(1));
    let expected = indoc! {"
        - id: A-2
          acceptance: [It also holds.]
    "};
    assert_eq!(row.fragment.as_str(), expected);
    ledger.remove(&queue().index(1).key("blocked_by").index(0));
    insta::assert_snapshot!(ledger.marked(), @"
    # -4 +0
     queue:
       - id: A-1
         acceptance: [It holds.]
     
    -  - id: A-2
    -    acceptance: [It also holds.]
    -
       - id: A-3
         blocked_by:
    -      - A-1
           - A-2
         acceptance: [It holds too.]
    ");
}

#[test]
fn a_blank_line_under_the_key_stays_when_the_first_row_goes() {
    let mut ledger = Ledger::new(indoc! {"
        active: A-1
        queue:

          - id: A-1

          - id: A-2
        archive: []
    "});
    ledger.take(&queue().index(0));
    insta::assert_snapshot!(ledger.marked(), @"
    # -2 +0
     active: A-1
     queue:
     
    -  - id: A-1
    -
       - id: A-2
     archive: []
    ");
}

#[test]
fn archiving_the_only_row_empties_the_queue_and_clears_the_pointer() {
    let mut ledger = Ledger::new(indoc! {"
        active: A-1

        queue:
          - id: A-1
            title: The only row
        archive: []
        horizon: []
    "});
    let row = ledger.take(&queue().index(0));
    ledger.put(&root().key("archive"), 0, &row);
    ledger.replace(&root().key("active"), &None::<&str>);
    insta::assert_snapshot!(ledger.marked(), @"
    # -3 +3
    -active: A-1
    +active: null
     
    -queue:
    +queue: []
    +archive:
       - id: A-1
         title: The only row
    -archive: []
     horizon: []
    ");
}

#[test]
fn a_parked_row_goes_into_a_section_added_at_the_end() {
    let mut ledger = Ledger::new(indoc! {"
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
    ledger.insert(&root(), "horizon", &NO_ITEMS, Position::End);
    let row = ledger.take(&queue().index(0));
    ledger.put(&horizon, 0, &row);
    ledger.remove(&parked.clone().key("blocked_by"));
    ledger.remove(&parked.clone().key("acceptance"));
    ledger.insert(&parked, "kind", "research", Position::End);
    ledger.insert(&parked, "open", "The fact is missing.", Position::End);
    insta::assert_snapshot!(ledger.marked(), @"
    # -5 +5
     active: null
    -queue:
    +queue: []
    +archive: []
    +horizon:
       - id: A-1
         outcome: The question has an answer.
    -    blocked_by: []
    -    acceptance:
    -      - It holds.
    -archive: []
    +    kind: research
    +    open: The fact is missing.
    ");
}

#[test]
fn a_promoted_row_carries_its_comment_across_sections() {
    let mut ledger = Ledger::new(indoc! {"
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
    .spaced(Spacing::Blank);
    let promoted = queue().index(1);
    let row = ledger.take(&root().key("horizon").index(0));
    ledger.put(&queue(), 1, &row);
    ledger.remove(&promoted.clone().key("kind"));
    ledger.remove(&promoted.clone().key("open"));
    ledger.insert(&promoted, "blocked_by", &["A-1"], Position::End);
    ledger.insert(&promoted, "acceptance", &["It holds."], Position::End);
    insta::assert_snapshot!(ledger.marked(), @"
    # -4 +7
     active: A-1
     
     queue:
       - id: A-1
         acceptance:
           - It holds.
    -archive: []
    -horizon:
    +
       # owns A-2
       - id: A-2
    -    kind: research
    -    open: The fact was missing.
         notes:
           - Keep this.
    +    blocked_by:
    +      - A-1
    +    acceptance:
    +      - It holds.
    +archive: []
    +horizon: []
    ");
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
        assert_eq!(Ledger::new(source).document.as_str(), source);
    }
    for source in [tidy, &untidy, unterminated] {
        let mut ledger = Ledger::new(source);
        ledger.replace(&root().key("active"), "A-2");
        assert_eq!(ledger.document.as_str(), source);
    }
}

#[test]
fn a_replaced_value_keeps_the_spaces_after_it() {
    let mut ledger = Ledger::new(&formatdoc! {"
        title: Row{trailing}
        next: 1
    ",
        trailing = "   ",
    });
    ledger.replace(&root().key("title"), "New");
    let expected = formatdoc! {"
        title: New{trailing}
        next: 1
    ",
        trailing = "   ",
    };
    assert_eq!(ledger.document.as_str(), expected);
}

#[test]
fn sorting_rows_moves_each_with_its_comment() {
    let mut ledger = Ledger::new(indoc! {"
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
    let row = ledger.take(&archive.clone().index(0));
    ledger.put(&archive, 2, &row);
    insta::assert_snapshot!(ledger.marked(), @"
    # -3 +3
     archive:
    -  - id: A-1
    -    completed: 2026-08-14T09:00:00
    -
       # owns A-3
       - id: A-3
         completed: 2026-08-16T23:08:28
     
       - id: A-2
         completed: 2026-08-15T12:30:00
    +
    +  - id: A-1
    +    completed: 2026-08-14T09:00:00
     horizon: []
    ");
}

#[test]
fn a_literal_note_becomes_a_list_of_paragraphs() {
    let mut ledger = Ledger::new(indoc! {"
        schema_version: 3
        queue:
          - id: A-1
            notes: |-
              First paragraph.

              Second paragraph.
        archive: []
    "});
    ledger.replace(&root().key("schema_version"), &4);
    ledger.replace(
        &queue().index(0).key("notes"),
        &["First paragraph.", "Second paragraph."],
    );
    insta::assert_snapshot!(ledger.marked(), @"
    # -5 +4
    -schema_version: 3
    +schema_version: 4
     queue:
       - id: A-1
    -    notes: |-
    -      First paragraph.
    -
    -      Second paragraph.
    +    notes:
    +      - First paragraph.
    +      - Second paragraph.
     archive: []
    ");
}

#[test]
fn a_row_moved_to_the_front_is_byte_identical() {
    let mut ledger = Ledger::new(indoc! {r#"
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
    let row = ledger.take(&queue().index(2));
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
    assert_eq!(row.fragment.as_str(), expected);
    ledger.put(&queue(), 0, &row);
    ledger.replace(&root().key("active"), "A-9");
    insta::assert_snapshot!(ledger.marked(), @r#"
    # -7 +7
    -active: A-2
    +active: A-9
     
     queue:
    -  - id: A-2
    -    outcome: Plain.
    -
    -  - id: A-3
    -    outcome: Plain.
    -
       - id: A-9
         acceptance:
           - A long line — with a dash and `code` that stays on one line however long it runs past any wrap width.
           - "`x` is quoted: with a colon"
         notes:
           - >-
             Folded note that
             wraps here.
     
    +  - id: A-2
    +    outcome: Plain.
    +
    +  - id: A-3
    +    outcome: Plain.
    +
     horizon:
       - id: A-7
    "#);
}

#[test]
fn an_empty_section_moves_above_the_comment_the_next_key_owns() {
    let mut ledger = Ledger::new(indoc! {"
        queue: []

        # Everything below this line is history, not work.
        archive:
          - id: A-1
        horizon: []
    "});
    ledger.remove(&root().key("horizon"));
    ledger.insert(&root(), "horizon", &NO_ITEMS, Position::Before("archive"));
    insta::assert_snapshot!(ledger.marked(), @"
    # -1 +1
     queue: []
     
    +horizon: []
     # Everything below this line is history, not work.
     archive:
       - id: A-1
    -horizon: []
    ");
}

#[test]
fn a_loose_comment_stays_above_a_key_inserted_below_it() {
    let mut ledger = Ledger::new(indoc! {"
        queue: []

        # A loose note.

        archive: []
    "});
    ledger.insert(&root(), "horizon", &NO_ITEMS, Position::Before("archive"));
    insta::assert_snapshot!(ledger.marked(), @"
    # -0 +1
     queue: []
     
     # A loose note.
     
    +horizon: []
     archive: []
    ");
}

#[test]
fn a_row_moved_into_an_empty_list_is_indented_to_it() {
    let mut ledger = Ledger::new(indoc! {"
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
    let row = ledger.take(&queue().index(0));
    ledger.put(&queue(), 0, &row);
    insta::assert_snapshot!(ledger.marked(), @"
    # -7 +7
     queue:
    -    # owns A-1
    -    - id: A-1
    -      outcome: >-
    -        Folded text
    -        on two lines.
    -      acceptance:
    -        - It holds.
    +  # owns A-1
    +  - id: A-1
    +    outcome: >-
    +      Folded text
    +      on two lines.
    +    acceptance:
    +      - It holds.
     archive: []
    ");
}

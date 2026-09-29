//! Paths, locations, styles, and comment ownership in the index.
#![allow(missing_docs)]

use indoc::indoc;
use yamled::{Document, Error, Path, Segment, Style};

const ROWS: &str = indoc! {r#"
    queue:
      # owns A-1
      - id: A-1
        title: "First: row"

      - id: A-2 # right
        title: Second
"#};

#[test]
fn a_nested_value_is_found_by_its_path() {
    let document = Document::parse(ROWS).unwrap();
    let title = document
        .node(&Path::root().key("queue").index(0).key("title"))
        .unwrap();
    assert_eq!(title.text(), r#""First: row""#);
    assert_eq!(title.style(), Style::DoubleQuoted);
    let key = title.key().unwrap();
    assert_eq!(&ROWS[key.start..key.end], "title");

    let queue = document.node(&Path::root().key("queue")).unwrap();
    assert_eq!(queue.style(), Style::BlockSequence);
    assert_eq!(queue.len(), 2);
}

#[test]
fn a_block_collection_ends_at_its_last_value() {
    let document = Document::parse(ROWS).unwrap();
    let first = document.node(&Path::root().key("queue").index(0)).unwrap();
    let expected = indoc! {r#"
        id: A-1
            title: "First: row"
    "#}
    .trim_end();
    assert_eq!(first.text(), expected);
}

#[test]
fn invalid_yaml_is_refused_with_its_location() {
    let source = indoc! {"
        a: 1
        b: [1, 2
        c: 3
    "};
    let Err(Error::Parse { line, .. }) = Document::parse(source) else {
        panic!("expected a parse error");
    };
    assert_eq!(line, 3);
}

#[test]
fn carriage_returns_and_several_documents_are_refused() {
    let windows = ["a: 1", "b: 2"].join("\r\n");
    assert!(matches!(
        Document::parse(windows),
        Err(Error::Unsupported { .. })
    ));
    let stream = indoc! {"
        a: 1
        ---
        b: 2
    "};
    assert!(matches!(
        Document::parse(stream),
        Err(Error::Unsupported { .. })
    ));
}

#[test]
fn a_json_pointer_decodes_its_escapes() {
    let path = Path::from_pointer("/a~1b/c~0d/0").unwrap();
    assert_eq!(
        path.segments(),
        &[
            Segment::Key("a/b".to_owned()),
            Segment::Key("c~d".to_owned()),
            Segment::Key("0".to_owned()),
        ]
    );
    assert_eq!(path.to_string(), "/a~1b/c~0d/0");
    assert_eq!(Path::from_pointer("").unwrap(), Path::root());
    assert!(matches!(
        Path::from_pointer("a"),
        Err(Error::Pointer { .. })
    ));
    assert!(matches!(
        Path::from_pointer("/a~2"),
        Err(Error::Pointer { .. })
    ));
}

#[test]
fn a_numeric_segment_is_an_index_on_a_sequence_and_a_key_on_a_mapping() {
    let source = indoc! {"
        list: [a, b]
        map:
          1: one
    "};
    let document = Document::parse(source).unwrap();
    let item = document
        .node(&Path::from_pointer("/list/1").unwrap())
        .unwrap();
    assert_eq!(item.text(), "b");
    let value = document.node(&Path::root().key("map").index(1)).unwrap();
    assert_eq!(value.text(), "one");
}

#[test]
fn a_schema_error_path_finds_its_line() {
    let document = Document::parse(ROWS).unwrap();
    let location = document
        .locate(&Path::from_pointer("/queue/1/title").unwrap())
        .unwrap();
    assert_eq!((location.line, location.column), (7, 12));
}

#[test]
fn a_missing_node_reports_its_nearest_ancestor() {
    let document = Document::parse(ROWS).unwrap();
    let pointer = Path::from_pointer("/queue/1/outcome").unwrap();
    assert!(document.locate(&pointer).is_none());
    let nearest = document.locate_nearest(&pointer).unwrap();
    assert_eq!((nearest.line, nearest.column), (6, 5));
}

#[test]
fn a_row_owns_the_comment_above_it_and_the_one_beside_it() {
    let document = Document::parse(ROWS).unwrap();
    let first = document
        .node(&Path::root().key("queue").index(0))
        .unwrap()
        .owned();
    assert!(ROWS[first.start..first.end].starts_with("  # owns A-1"));
    let second = document
        .node(&Path::root().key("queue").index(1))
        .unwrap()
        .owned();
    assert!(ROWS[second.start..second.end].contains("# right"));
    assert!(!ROWS[second.start..second.end].contains("# owns"));
}

#[test]
fn a_hash_inside_a_block_scalar_is_not_a_comment() {
    let source = indoc! {"
        notes: |-
          # heading, not a comment
        next: 1
    "};
    let document = Document::parse(source).unwrap();
    let next = document.node(&Path::root().key("next")).unwrap().owned();
    let expected = indoc! {"
        next: 1
    "};
    assert_eq!(&source[next.start..next.end], expected);
}

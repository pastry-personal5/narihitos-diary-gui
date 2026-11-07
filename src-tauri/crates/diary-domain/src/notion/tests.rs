use super::*;

fn kinds(p: &Projection) -> Vec<&'static str> {
    p.blocks.iter().map(|b| b.kind.tag()).collect()
}

#[test]
fn supported_blocks_project_in_order() {
    let p = project(
        "# Title\n\nHello *world*.\n\n- a\n- b\n\n1. one\n\n> quote\n\n```rust\nfn x() {}\n```\n\n---\n",
    );
    assert_eq!(
        kinds(&p),
        [
            "heading_1",
            "paragraph",
            "bulleted_list_item",
            "bulleted_list_item",
            "numbered_list_item",
            "quote",
            "code",
            "divider"
        ]
    );
    assert_eq!(p.blocks[1].text, "Hello *world*.");
    assert_eq!(p.blocks[6].text, "fn x() {}");
    assert_eq!(p.blocks[6].kind, Kind::Code("rust".into()));
    assert_eq!(p.warnings, 0);
}

#[test]
fn projection_is_deterministic() {
    let body = "# A\n\ntext\n\n- x\n  - y\n";
    assert_eq!(project(body), project(body));
    assert_eq!(
        canonical(&project(body).blocks),
        canonical(&project(body).blocks)
    );
}

#[test]
fn unsupported_markdown_stays_literal_and_is_counted() {
    let p = project("| a | b |\n|---|---|\n| 1 | 2 |\n\n<div>raw</div>\n");
    assert_eq!(p.warnings, 2);
    assert!(p.blocks.iter().all(|b| b.kind == Kind::Paragraph));
    assert!(p.blocks[0].text.contains("| a | b |"));
    assert!(p.blocks[1].text.contains("<div>raw</div>"));
}

#[test]
fn deep_headings_and_nesting_are_restructured_with_a_warning() {
    let p = project("#### Four\n\n- a\n  - b\n    - c\n");
    assert_eq!(p.blocks[0].kind, Kind::Heading(3));
    assert_eq!(p.warnings, 2);
    let list = &p.blocks[1];
    assert_eq!(list.children.len(), 2);
    assert_eq!(list.children[0].text, "b");
    assert_eq!(list.children[1].text, "  c");
    assert!(list.children.iter().all(|c| c.children.is_empty()));
}

#[test]
fn rich_text_splits_on_scalar_boundaries() {
    let text: String = "한".repeat(1_999) + "😀" + &"x".repeat(10);
    let pieces = split_rich(&text);
    assert!(
        pieces
            .iter()
            .all(|p| p.encode_utf16().count() <= MAX_RICH_UNITS)
    );
    assert_eq!(pieces.concat(), text);
    assert_eq!(split_rich(""), [""]);
}

#[test]
fn oversize_text_continues_in_following_blocks() {
    let long = "a ".repeat(120_000);
    let p = project(&long);
    assert!(p.blocks.len() > 1);
    assert_eq!(p.warnings, 1);
    assert!(
        p.blocks
            .iter()
            .all(|b| b.text.encode_utf16().count() <= MAX_BLOCK_UNITS)
    );
}

#[test]
fn chunks_respect_count_blocks_and_bytes() {
    let blocks: Vec<Block> = (0..250)
        .map(|i| Block::leaf(Kind::Paragraph, format!("p{i}")))
        .collect();
    let chunks = chunk(&blocks, |b| b.text.len(), usize::MAX);
    assert_eq!(
        chunks.iter().map(Vec::len).collect::<Vec<_>>(),
        [100, 100, 50]
    );
    let chunks = chunk(&blocks, |_| 100, 1_000);
    assert!(chunks.iter().all(|c| c.len() <= 10));
    assert_eq!(chunks.concat(), blocks);
    assert!(chunk(&[], |_| 1, 10).is_empty());
}

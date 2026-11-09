use super::*;

struct Fnv;
impl Hasher for Fnv {
    fn sha256_hex(&self, bytes: &[u8]) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in bytes {
            h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
        }
        format!("{h:016x}{:016x}", bytes.len())
    }
}

fn texts(src: &str) -> Vec<&str> {
    units(src)
        .unwrap()
        .into_iter()
        .map(|u| &src[u.range])
        .collect()
}

fn all(h: &[Hunk]) -> BTreeSet<String> {
    h.iter().map(|x| x.id.clone()).collect()
}

fn sets(ids: &[&Hunk]) -> BTreeSet<String> {
    ids.iter().map(|h| h.id.clone()).collect()
}

#[test]
fn t01_units_are_byte_exact_and_structures_indivisible() {
    let cases = [
        "",
        "\n\n  lead\n\nA soft\nwrapped line\n\nB\n",
        "# Title\n\nbody\n\n- a\n  cont\n  - nested\n- b\n\n> quote\n> more\n\n",
        "```\nx\n\n# not a heading\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nend",
        "A\r\n\r\nB\r\n",
        "```\nunclosed fence runs on\n\nstill code",
        "[ref]: http://x\n\nA\n\n[other]: http://y\n",
    ];
    for src in cases {
        let u = units(src).unwrap();
        assert_eq!(
            u.iter().map(|u| &src[u.range.clone()]).collect::<String>(),
            src,
            "{src:?}"
        );
    }
    let src = "# Title\n\nbody\n\n- a\n  cont\n  - nested\n- b\n\n```\nx\n\n# no\n```\n";
    assert_eq!(
        texts(src),
        [
            "# Title\n\n",
            "body\n\n",
            "- a\n  cont\n  - nested\n",
            "- b\n\n",
            "```\nx\n\n# no\n```\n"
        ]
    );
    let u = units(src).unwrap();
    assert_eq!(u[0].kind, UnitKind::Heading);
    assert_eq!(u[2].kind, UnitKind::ListItem);
    assert_eq!(u[4].kind, UnitKind::Fence);
    assert_eq!(
        texts("\n\n  lead\n\nA soft\nwrapped\n"),
        ["\n\n  ", "lead\n\n", "A soft\nwrapped\n"]
    );
    // A link reference definition rides along with the preceding unit.
    assert_eq!(texts("A\n\n[r]: http://x\n"), ["A\n\n[r]: http://x\n"]);
}

#[test]
fn t01_unverifiable_spans_are_unsupported() {
    let text = "ab cd";
    let p = |r| Unit {
        kind: UnitKind::Paragraph,
        range: r,
    };
    for bad in [
        vec![p(0..2), p(3..5)],
        vec![p(0..3), p(2..5)],
        vec![p(0..4)],
    ] {
        assert_eq!(verify(text, &bad), Err(ReviewError::Unsupported));
    }
    assert_eq!(verify("é", &[p(0..1)]), Err(ReviewError::Unsupported));
}

#[test]
fn t02_contract_examples_and_ties() {
    let (b, c) = ("A\n\nB\n\nC\n", "A\n\nB2\n\nX\n\nC\n");
    let h = align(&Fnv, b, c).unwrap();
    assert_eq!(
        h.iter().map(|x| x.kind).collect::<Vec<_>>(),
        [HunkKind::Replace, HunkKind::Insert]
    );
    assert_eq!(compose(b, c, &h, &sets(&[&h[1]])), "A\n\nB\n\nX\n\nC\n");
    assert_eq!(compose(b, c, &h, &sets(&[&h[0]])), "A\n\nB2\n\nC\n");

    let (b, c) = ("A\n\nB\n\nC\n", "A\n\nC\n");
    let h = align(&Fnv, b, c).unwrap();
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].kind, HunkKind::Delete);
    assert_eq!(compose(b, c, &h, &all(&h)), c);

    // Earliest baseline index wins repeated paragraphs.
    let (b, c) = ("A\n\nA\n\n", "A\n\n");
    let h = align(&Fnv, b, c).unwrap();
    assert_eq!((h.len(), h[0].baseline.clone()), (1, 3..6));
    assert_eq!(align(&Fnv, b, c).unwrap(), h);

    let h = align(&Fnv, "", "new\n").unwrap();
    assert_eq!((h.len(), h[0].kind), (1, HunkKind::Insert));
    let h = align(&Fnv, "old\n", "").unwrap();
    assert_eq!((h.len(), h[0].kind), (1, HunkKind::Delete));
    assert!(align(&Fnv, "same\n", "same\n").unwrap().is_empty());
}

#[test]
fn t02_limits() {
    let big = "p\n\n".repeat(4001);
    assert_eq!(align(&Fnv, &big, "x\n"), Err(ReviewError::Limit));
    let half = (0..1000).map(|i| format!("p{i}\n\n")).collect::<String>();
    assert!(align(&Fnv, &half, &half).is_ok());
}

#[test]
fn t03_compose_exactness() {
    let fixtures = [
        ("", "A\n\nB\n"),
        ("A\n\nB\n", ""),
        ("A\n\nB", "A\n\nB\n\n"),
        (
            "# H\n\n- a\n- b\n\ntext\n",
            "# H2\n\n- a\n- c\n- d\n\ntext\n\nmore\n",
        ),
        ("A\n\nB\n\nC\n", "C\n\nB\n\nA\n"),
        ("same  \n", "same \n"),
    ];
    for (b, c) in fixtures {
        let h = align(&Fnv, b, c).unwrap();
        assert_eq!(compose(b, c, &h, &all(&h)), c, "{b:?}->{c:?}");
        assert_eq!(compose(b, c, &h, &BTreeSet::new()), b);
        for one in &h {
            let (body, pending) = view(b, c, &h, &sets(&[one]), &BTreeSet::new());
            assert_eq!(pending.len(), h.len() - 1);
            assert!(pending.iter().all(|p| p.body.end <= body.len()));
        }
        if let Some(first) = h.first() {
            let (_, p) = view(b, c, &h, &BTreeSet::new(), &sets(&[first]));
            assert!(p.iter().all(|p| p.id != first.id));
        }
    }
}

#[test]
fn view_locates_pending_in_current_body() {
    let (b, c) = ("A\n\nB\n\nC\n", "A\n\nB2\n\nX\n\nC\n");
    let h = align(&Fnv, b, c).unwrap();
    let (body, p) = view(b, c, &h, &sets(&[&h[1]]), &BTreeSet::new());
    assert_eq!(body, "A\n\nB\n\nX\n\nC\n");
    assert_eq!(&body[p[0].body.clone()], "B\n\n");
    assert_eq!(p[0].text, "B2\n\n");
}

#[test]
fn set_digest_is_order_free_and_empty_defined() {
    let a = BTreeSet::from(["b".to_owned(), "a".to_owned()]);
    let b = BTreeSet::from(["a".to_owned(), "b".to_owned()]);
    assert_eq!(set_digest(&Fnv, &a), set_digest(&Fnv, &b));
    assert_ne!(set_digest(&Fnv, &a), set_digest(&Fnv, &BTreeSet::new()));
}

#[test]
fn appended_paragraph_owns_missing_separator() {
    for baseline in ["P1\n\nP2", "P1\n\nP2\n"] {
        let candidate = "P1\n\nP2\n\nP3";
        let hunks = align(&Fnv, baseline, candidate).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].kind, HunkKind::Insert);
        assert_eq!(
            compose(baseline, candidate, &hunks, &all(&hunks)),
            candidate
        );
        assert_eq!(
            compose(baseline, candidate, &hunks, &BTreeSet::new()),
            baseline
        );
    }
}

#[test]
fn later_append_keeps_boundary_when_earlier_append_is_rejected() {
    let baseline = "P1\n\nP2";
    let candidate = "P1\n\nP2\n\nP3\n\nP4";
    let hunks = align(&Fnv, baseline, candidate).unwrap();
    assert_eq!(hunks.len(), 2);
    assert_eq!(
        compose(baseline, candidate, &hunks, &sets(&[&hunks[1]])),
        "P1\n\nP2\n\nP4"
    );
    assert_eq!(
        compose(baseline, candidate, &hunks, &all(&hunks)),
        candidate
    );
}

#[test]
fn append_subsets_preserve_unicode_and_crlf_boundaries() {
    let baseline = "처음\r\n\r\n마지막";
    let candidate = "처음\r\n\r\n마지막\r\n\r\n셋째\r\n\r\n넷째";
    let hunks = align(&Fnv, baseline, candidate).unwrap();
    assert_eq!(hunks.len(), 2);
    for mask in 0..4 {
        let accepted = hunks
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, h)| h.id.clone())
            .collect();
        let result = compose(baseline, candidate, &hunks, &accepted);
        assert!(result.starts_with(baseline));
        if mask & 2 != 0 {
            assert!(result.contains("\r\n\r\n넷째"));
        }
        if mask == 3 {
            assert_eq!(result, candidate);
        }
        if mask == 0 {
            assert_eq!(result, baseline);
        }
    }
}

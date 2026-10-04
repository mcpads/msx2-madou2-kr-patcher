use super::*;

#[test]
fn matches_source_wording_and_rejects_conflicts_or_cell_overflow() {
    let source = b"A\xff";
    let reference = Reference {
        id: "banked-001".into(),
        offset: 0,
        end: 2,
    };
    let names = BTreeSet::from([reference.id.clone()]);
    let originals = BTreeMap::from([("banked-001", &reference)]);
    let wordings = BTreeMap::from([("banked-001", "약초<END>")]);
    for (text, valid) in [
        ("약초", true),
        ("붕대", false),
        ("", false),
        ("가나다라마바사아자차카", false),
    ] {
        let draft = Draft {
            source_sha256: sha256(source),
            policy: "non-distribution-development".into(),
            entries: vec![Entry {
                id: reference.id.clone(),
                source_raw_sha256: sha256(source),
                text: text.into(),
                review: "needs_review".into(),
            }],
        };
        assert_eq!(
            validate_names(source, draft, &names, &originals, &wordings).is_ok(),
            valid
        );
    }
}

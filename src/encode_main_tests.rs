use super::*;
use serde_json::json;

#[test]
fn draft_bytes_must_match_the_source_and_have_unique_span_ids() {
    let source = b"\0A\xff\0";
    let draft = json!({
        "source_sha256": sha256(source),
        "policy": "non-distribution-development",
        "control_policy": "preserve_flow_and_delays_decode_source_kana",
        "entries": [{"id": "text-00001", "text": "가<END>", "references": ["fixture-0"],
            "review": "needs_review", "context": "Test dialogue",
            "source": {"disk_offset": 1, "raw_hex": "41ff", "raw_sha256": sha256(b"A\xff"), "annotated_source": "A<END>"}}]
    });
    let refs = [Reference {
        id: "fixture-0".into(),
        offset: 1,
        end: 3,
    }];
    let report = encode_references(source, &serde_json::to_vec(&draft).unwrap(), &refs).unwrap();
    assert_eq!(report.entries[0].encoded_hex, "05ff");
    for mutation in 0..7 {
        let mut bad = draft.clone();
        match mutation {
            0 => bad["entries"][0]["source"]["raw_hex"] = json!("42ff"),
            1 => bad["entries"][0]["source"]["disk_offset"] = json!(100),
            2 => bad["entries"][0]["id"] = json!("text-00002"),
            3 => {
                let entry = bad["entries"][0].clone();
                bad["entries"].as_array_mut().unwrap().push(entry);
            }
            4 => bad["entries"][0]["references"] = json!(["wrong"]),
            5 => bad["entries"][0]["source"]["annotated_source"] = json!("B<END>"),
            6 => bad["entries"] = json!([]),
            _ => unreachable!(),
        }
        assert!(encode_references(source, &serde_json::to_vec(&bad).unwrap(), &refs).is_err());
    }
}

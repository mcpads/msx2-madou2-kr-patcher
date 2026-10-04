use super::*;

fn fixture() -> (Vec<u8>, Vec<AnalysisWrite>, Vec<ImageRegion>) {
    (
        b"012345".to_vec(),
        vec![AnalysisWrite {
            name: "text".into(),
            offset: 2,
            expected_hex: hex::encode(b"23"),
            replacement_hex: hex::encode(b"AB"),
        }],
        vec![ImageRegion {
            id: "text".into(),
            range: 2..4,
            kind: RegionKind::Data,
            reason: "Small independent resident pool".into(),
        }],
    )
}

#[test]
fn applies_complete_writer_and_audits_protected_bytes() {
    let (source, writes, regions) = fixture();
    let plan = compile_writes(&source, &writes, regions).unwrap();
    assert_eq!(plan.apply(&source, None).unwrap(), b"01AB45");
    assert!(plan.audit(&source, b"X1AB45", None).is_err());
    assert!(plan.apply(b"01XX45", None).is_err());
}

#[test]
fn rejects_missing_unknown_duplicate_and_partial_writers() {
    for mutation in 0..4 {
        let (source, mut writes, regions) = fixture();
        match mutation {
            0 => writes.clear(),
            1 => writes[0].name = "unowned".into(),
            2 => writes.push(AnalysisWrite {
                name: "text".into(),
                offset: 2,
                expected_hex: hex::encode(b"23"),
                replacement_hex: hex::encode(b"AB"),
            }),
            3 => {
                writes[0].expected_hex = hex::encode(b"2");
                writes[0].replacement_hex = hex::encode(b"A");
            }
            _ => unreachable!(),
        }
        assert!(compile_writes(&source, &writes, regions).is_err());
    }
}

#[test]
fn rejects_shifted_writer_and_executable_intent() {
    let (source, mut writes, regions) = fixture();
    writes[0].offset = 1;
    assert!(compile_writes(&source, &writes, regions).is_err());
    let (source, writes, mut regions) = fixture();
    regions[0].kind = RegionKind::MachineCode;
    assert!(compile_writes(&source, &writes, regions).is_err());
}

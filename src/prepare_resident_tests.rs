use super::*;

#[test]
fn shared_starts_share_pointers_but_distinct_tails_get_independent_text() {
    let source = [0; 20];
    let table = Table {
        name: "fixture",
        offset: 0,
        count: 3,
        resolved: 3,
        base: 0x3000,
        end: 14,
    };
    let refs = [
        Reference {
            id: "fixture-000".into(),
            offset: 8,
            end: 13,
        },
        Reference {
            id: "fixture-001".into(),
            offset: 8,
            end: 13,
        },
        Reference {
            id: "fixture-002".into(),
            offset: 10,
            end: 13,
        },
    ];
    let refs = refs.iter().collect::<Vec<_>>();
    let mut encoded = BTreeMap::from([(8, vec![5, 255]), (10, vec![6, 7, 255])]);
    let (writes, budget) = pool_writes(&source, &table, &refs, &encoded).unwrap();
    assert_eq!(writes[0].replacement_hex, "05ff0607ff000000");
    assert_eq!(writes[1].replacement_hex, "0630");
    assert_eq!(writes[2].replacement_hex, "0630");
    assert_eq!(writes[3].replacement_hex, "0830");
    assert_eq!(budget.remaining, 3);
    encoded.insert(10, vec![7; 7]);
    assert!(pool_writes(&source, &table, &refs, &encoded).is_err());
}

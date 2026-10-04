use super::*;
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, RegionKind, WriteIntent, WritePlan,
};
use retro_z80::{Assembler, Instruction};

#[test]
fn code_write_requires_matching_source_profile_placement_and_bytes() {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Jp(0x1234));
    let program = assembler.assemble(0x8000).unwrap();
    let replacement = program.bytes().to_vec();
    let verifier = Verifier(vec![CodeSource {
        id: "jump".into(),
        disk_offset: 0,
        program,
    }]);
    let baseline = [0; 3];
    let write = ExpectedWrite {
        id: "entry".into(),
        owner: "test".into(),
        purpose: "Jump installation".into(),
        offset: 0,
        expected_original: baseline.to_vec(),
        replacement: replacement.clone(),
        intent: WriteIntent::MachineCode(MachineCodeProvenance {
            assembly_source_id: "jump".into(),
            isa_profile_id: PROFILE_ID.into(),
        }),
    };
    let apply = |write| {
        WritePlan::new()
            .region(ImageRegion {
                id: "code".into(),
                range: 0..3,
                kind: RegionKind::MachineCode,
                reason: "Test entry".into(),
            })
            .write(write)
            .apply(&baseline, Some(&verifier))
    };
    assert_eq!(apply(write.clone()).unwrap(), replacement);
    let mut altered = write.clone();
    altered.replacement[1] ^= 1;
    assert!(apply(altered).is_err());
    for (source_id, profile) in [("unknown", PROFILE_ID), ("jump", "wrong-isa")] {
        let mut altered = write.clone();
        altered.intent = WriteIntent::MachineCode(MachineCodeProvenance {
            assembly_source_id: source_id.into(),
            isa_profile_id: profile.into(),
        });
        assert!(apply(altered).is_err());
    }
    let mut altered = write;
    altered.offset = 1;
    assert!(apply(altered).is_err());
}

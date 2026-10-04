//! Expected Write adapter bound to typed sources and their disk/CPU placement.
use expected_write::{
    DecodedInstruction, MachineCodeCheck, MachineCodeVerifier, MachineCodeVerifierError,
};
use retro_z80::{AssembledProgram, PROFILE_ID, decode_bytes, encode_bytes, verify_placed_program};

pub struct CodeSource {
    pub id: String,
    pub disk_offset: usize,
    pub program: AssembledProgram,
}

pub struct Verifier(pub Vec<CodeSource>);

fn error(e: impl std::fmt::Display) -> MachineCodeVerifierError {
    MachineCodeVerifierError::new(e.to_string())
}

impl Verifier {
    fn source(
        &self,
        check: &MachineCodeCheck<'_>,
    ) -> Result<&AssembledProgram, MachineCodeVerifierError> {
        if check.provenance.isa_profile_id != PROFILE_ID {
            return Err(error("Wrong Z80 profile"));
        }
        let source = self
            .0
            .iter()
            .find(|s| s.id == check.provenance.assembly_source_id)
            .ok_or_else(|| error("Unknown typed assembly source"))?;
        if check.write.offset != source.disk_offset {
            return Err(error(
                "Machine code disk placement differs from typed source",
            ));
        }
        Ok(&source.program)
    }
}

impl MachineCodeVerifier for Verifier {
    fn assemble_source(
        &self,
        check: &MachineCodeCheck<'_>,
    ) -> Result<Vec<u8>, MachineCodeVerifierError> {
        Ok(self.source(check)?.bytes().to_vec())
    }

    fn disassemble(
        &self,
        check: &MachineCodeCheck<'_>,
    ) -> Result<Vec<DecodedInstruction>, MachineCodeVerifierError> {
        let program = self.source(check)?;
        let actual =
            verify_placed_program(&check.write.replacement, program.origin()).map_err(error)?;
        if actual != program.instructions() {
            return Err(error(
                "Decoded instructions differ from placed typed source",
            ));
        }
        let mut result = Vec::new();
        let mut offset = 0;
        for instruction in actual {
            let decoded = decode_bytes(&check.write.replacement[offset..]).map_err(error)?;
            result.push(DecodedInstruction {
                offset,
                len: decoded.length(),
                canonical: instruction.to_string(),
            });
            offset += decoded.length();
        }
        Ok(result)
    }

    fn assemble_decoded(
        &self,
        check: &MachineCodeCheck<'_>,
        instructions: &[DecodedInstruction],
    ) -> Result<Vec<u8>, MachineCodeVerifierError> {
        if self.disassemble(check)? != instructions {
            return Err(error("Decode records changed before reassembly"));
        }
        let mut result = Vec::new();
        for record in instructions {
            let decoded = decode_bytes(&check.write.replacement[record.offset..]).map_err(error)?;
            result.extend(encode_bytes(decoded.instruction()).map_err(error)?);
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "z80_writes_tests.rs"]
mod tests;

use crate::cli::{Cli, ContractCommand};
use crate::LatchError;

pub fn handle(_cmd: &ContractCommand, _cli: &Cli) -> Result<(), LatchError> {
    // Stub: Bjarn will implement contracts
    Err(LatchError::Validation("contracts module not yet implemented".into()))
}

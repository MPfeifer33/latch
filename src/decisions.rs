use crate::cli::{Cli, DecisionCommand};
use crate::LatchError;

pub fn handle(_cmd: &DecisionCommand, _cli: &Cli) -> Result<(), LatchError> {
    // Stub: Bjarn will implement decisions
    Err(LatchError::Validation("decisions module not yet implemented".into()))
}

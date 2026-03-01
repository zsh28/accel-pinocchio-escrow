pub mod r#unsafe;
pub mod wincode;

use pinocchio::error::ProgramError;
pub use r#unsafe::*;
pub use wincode::*;

pub enum EscrowInstrctions {
    Make = 0,
    Take = 1,
    Cancel = 2,
    MakeV2 = 3,
    TakeV2 = 4,
    CancelV2 = 5,
}

impl TryFrom<&u8> for EscrowInstrctions {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(EscrowInstrctions::Make),
            1 => Ok(EscrowInstrctions::Take),
            2 => Ok(EscrowInstrctions::Cancel),
            3 => Ok(EscrowInstrctions::MakeV2),
            4 => Ok(EscrowInstrctions::TakeV2),
            5 => Ok(EscrowInstrctions::CancelV2),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

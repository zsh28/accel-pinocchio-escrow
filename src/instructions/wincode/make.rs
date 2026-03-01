use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_pubkey::derive_address;
use pinocchio_system::instructions::CreateAccount;
use wincode::SchemaRead;

use crate::state::Escrow;

#[derive(SchemaRead)]
pub struct MakeInstructionDataV2 {
    pub amount_to_receive: u64,
    pub amount_to_give: u64,
    pub bump: u8,
}

pub fn process_make_instruction_v2(accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    let [maker, mint_a, mint_b, escrow_account, maker_ata, escrow_ata, system_program, token_program, _associated_token_program @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !maker.is_signer() {
        return Err(ProgramError::IncorrectAuthority);
    }

    let ix_data = wincode::deserialize::<MakeInstructionDataV2>(data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    {
        let maker_ata_state = pinocchio_token::state::TokenAccount::from_account_view(maker_ata)?;
        if maker_ata_state.owner() != maker.address() {
            return Err(ProgramError::IllegalOwner);
        }
        if maker_ata_state.mint() != mint_a.address() {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    let bump = ix_data.bump;
    let seed = [b"escrow".as_ref(), maker.address().as_ref(), &[bump]];
    let escrow_account_pda = derive_address(&seed, None, crate::ID.as_array());

    if escrow_account_pda != *escrow_account.address().as_array() {
        return Err(ProgramError::InvalidAccountData);
    }

    let bump_seed = [bump];
    let signer_seed = [
        Seed::from(b"escrow"),
        Seed::from(maker.address().as_array()),
        Seed::from(&bump_seed),
    ];

    CreateAccount {
        from: maker,
        to: escrow_account,
        lamports: Rent::get()?.try_minimum_balance(Escrow::LEN)?,
        space: Escrow::LEN as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&[Signer::from(&signer_seed)])?;

    {
        let escrow_data = unsafe { escrow_account.borrow_unchecked_mut() };
        let escrow_state = Escrow::load_mut(escrow_data)?;

        escrow_state.set_maker(maker.address());
        escrow_state.set_mint_a(mint_a.address());
        escrow_state.set_mint_b(mint_b.address());
        escrow_state.set_amount_to_receive(ix_data.amount_to_receive);
        escrow_state.set_amount_to_give(ix_data.amount_to_give);
        escrow_state.bump = bump;
    }

    pinocchio_associated_token_account::instructions::Create {
        funding_account: maker,
        account: escrow_ata,
        wallet: escrow_account,
        mint: mint_a,
        token_program,
        system_program,
    }
    .invoke()?;

    pinocchio_token::instructions::Transfer {
        from: maker_ata,
        to: escrow_ata,
        authority: maker,
        amount: ix_data.amount_to_give,
    }
    .invoke()?;

    Ok(())
}

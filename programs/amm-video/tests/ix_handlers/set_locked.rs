use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_pubkey::Pubkey,
};

pub fn create_set_locked_ix(authority: Pubkey, config: Pubkey, locked: bool) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::SetLocked { locked }.data(),
        amm_video::accounts::SetLocked { authority, config }.to_account_metas(None),
    )
}

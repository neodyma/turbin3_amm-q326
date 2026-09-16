use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    litesvm_token::spl_token::ID as TOKEN_PROGRAM_ID,
    solana_pubkey::Pubkey,
};

use super::PoolAccounts;

#[allow(clippy::too_many_arguments)]
pub fn create_swap_ix(
    user: Pubkey,
    pool: &PoolAccounts,
    user_x: Pubkey,
    user_y: Pubkey,
    is_x: bool,
    amount_in: u64,
    min_amount_out: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Swap {
            is_x,
            amount_in,
            min_amount_out,
        }
        .data(),
        amm_video::accounts::Swap {
            user,
            mint_x: pool.mint_x,
            mint_y: pool.mint_y,
            config: pool.config,
            treasury: pool.treasury,
            vault_x: pool.vault_x,
            vault_y: pool.vault_y,
            user_x,
            user_y,
            treasury_x: pool.treasury_x,
            treasury_y: pool.treasury_y,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}

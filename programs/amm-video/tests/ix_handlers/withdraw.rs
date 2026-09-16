use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    litesvm_token::spl_token::ID as TOKEN_PROGRAM_ID,
    solana_pubkey::Pubkey,
};

use super::PoolAccounts;

#[allow(clippy::too_many_arguments)]
pub fn create_withdraw_ix(
    user: Pubkey,
    pool: &PoolAccounts,
    user_x: Pubkey,
    user_y: Pubkey,
    user_lp: Pubkey,
    amount: u64,
    min_x: u64,
    min_y: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Withdraw {
            amount,
            min_x,
            min_y,
        }
        .data(),
        amm_video::accounts::Withdraw {
            user,
            mint_x: pool.mint_x,
            mint_y: pool.mint_y,
            config: pool.config,
            mint_lp: pool.mint_lp,
            vault_x: pool.vault_x,
            vault_y: pool.vault_y,
            user_x,
            user_y,
            user_lp,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}

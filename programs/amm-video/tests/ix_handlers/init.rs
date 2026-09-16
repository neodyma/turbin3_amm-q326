use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::associated_token::ID as ASSOCIATED_TOKEN_PROGRAM_ID,
    litesvm_token::spl_token::ID as TOKEN_PROGRAM_ID,
    solana_pubkey::Pubkey,
};

use super::PoolAccounts;

pub fn create_initialize_ix(
    initializer: Pubkey,
    pool: &PoolAccounts,
    seed: u64,
    fee: u16,
    authority: Option<Pubkey>,
) -> Instruction {
    Instruction::new_with_bytes(
        amm_video::id(),
        &amm_video::instruction::Initialize {
            seed,
            fee,
            authority,
        }
        .data(),
        amm_video::accounts::Initialize {
            initializer,
            treasury: pool.treasury,
            mint_x: pool.mint_x,
            mint_y: pool.mint_y,
            mint_lp: pool.mint_lp,
            vault_x: pool.vault_x,
            vault_y: pool.vault_y,
            treasury_x: pool.treasury_x,
            treasury_y: pool.treasury_y,
            config: pool.config,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}

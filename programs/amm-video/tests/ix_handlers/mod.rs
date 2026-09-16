use anchor_spl::associated_token;
use solana_pubkey::Pubkey;

pub mod deposit;
pub mod init;
pub mod set_locked;
pub mod swap;
pub mod withdraw;

pub use deposit::*;
pub use init::*;
pub use set_locked::*;
pub use swap::*;
pub use withdraw::*;

#[derive(Clone, Copy)]
pub struct PoolAccounts {
    pub mint_x: Pubkey,
    pub mint_y: Pubkey,
    pub config: Pubkey,
    pub mint_lp: Pubkey,
    pub vault_x: Pubkey,
    pub vault_y: Pubkey,
    pub treasury: Pubkey,
    pub treasury_x: Pubkey,
    pub treasury_y: Pubkey,
}

impl PoolAccounts {
    pub fn derive(seed: u64, mint_x: Pubkey, mint_y: Pubkey, treasury: Pubkey) -> Self {
        let config = Pubkey::find_program_address(
            &[b"config", seed.to_le_bytes().as_ref()],
            &amm_video::id(),
        )
        .0;
        let mint_lp = Pubkey::find_program_address(&[b"lp", config.as_ref()], &amm_video::id()).0;

        Self {
            mint_x,
            mint_y,
            config,
            mint_lp,
            vault_x: associated_token::get_associated_token_address(&config, &mint_x),
            vault_y: associated_token::get_associated_token_address(&config, &mint_y),
            treasury,
            treasury_x: associated_token::get_associated_token_address(&treasury, &mint_x),
            treasury_y: associated_token::get_associated_token_address(&treasury, &mint_y),
        }
    }
}

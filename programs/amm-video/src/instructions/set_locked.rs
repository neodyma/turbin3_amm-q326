use anchor_lang::prelude::*;

use crate::{error::AmmError, state::Config};

#[derive(Accounts)]
pub struct SetLocked<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Account<'info, Config>,
}

impl<'info> SetLocked<'info> {
    pub fn set_locked(&mut self, locked: bool) -> Result<()> {
        let Some(authority) = self.config.authority else {
            return err!(AmmError::NoAuthoritySet);
        };

        require_keys_eq!(authority, self.authority.key(), AmmError::InvalidAuthority);

        self.config.locked = locked;

        Ok(())
    }
}

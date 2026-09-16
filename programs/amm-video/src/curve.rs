use anchor_lang::prelude::*;

use crate::error::AmmError;

const BASIS_POINTS: u128 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAmounts {
    pub x: u64,
    pub y: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwapAmounts {
    pub net_input: u64,
    pub fee: u64,
    pub output: u64,
}

pub struct ConstantProduct;

impl ConstantProduct {
    pub fn deposit_amounts(
        reserve_x: u64,
        reserve_y: u64,
        lp_supply: u64,
        lp_amount: u64,
    ) -> Result<TokenAmounts> {
        require!(
            reserve_x > 0 && reserve_y > 0 && lp_supply > 0,
            AmmError::NoLiquidityInPool
        );
        require!(lp_amount > 0, AmmError::InvalidAmount);

        Ok(TokenAmounts {
            x: checked_mul_div_ceil(reserve_x, lp_amount, lp_supply)?,
            y: checked_mul_div_ceil(reserve_y, lp_amount, lp_supply)?,
        })
    }

    pub fn withdraw_amounts(
        reserve_x: u64,
        reserve_y: u64,
        lp_supply: u64,
        lp_amount: u64,
    ) -> Result<TokenAmounts> {
        require!(
            reserve_x > 0 && reserve_y > 0 && lp_supply > 0,
            AmmError::NoLiquidityInPool
        );
        require!(lp_amount > 0, AmmError::InvalidAmount);
        require!(lp_amount <= lp_supply, AmmError::InsufficientBalance);

        Ok(TokenAmounts {
            x: checked_mul_div_floor(reserve_x, lp_amount, lp_supply)?,
            y: checked_mul_div_floor(reserve_y, lp_amount, lp_supply)?,
        })
    }

    pub fn swap(
        reserve_in: u64,
        reserve_out: u64,
        gross_input: u64,
        fee_basis_points: u16,
        minimum_output: u64,
    ) -> Result<SwapAmounts> {
        require!(fee_basis_points < 10_000, AmmError::FeePercentErr);
        require!(
            reserve_in > 0 && reserve_out > 0,
            AmmError::NoLiquidityInPool
        );
        require!(gross_input > 0, AmmError::InvalidAmount);

        let net_input = u64::try_from(
            u128::from(gross_input)
                .checked_mul(BASIS_POINTS - u128::from(fee_basis_points))
                .ok_or_else(|| error!(AmmError::Overflow))?
                .checked_div(BASIS_POINTS)
                .ok_or_else(|| error!(AmmError::Underflow))?,
        )
        .map_err(|_| error!(AmmError::Overflow))?;
        require!(net_input > 0, AmmError::InvalidAmount);

        let fee = gross_input
            .checked_sub(net_input)
            .ok_or_else(|| error!(AmmError::Underflow))?;
        let denominator = reserve_in
            .checked_add(net_input)
            .ok_or_else(|| error!(AmmError::Overflow))?;
        let output = checked_mul_div_floor(reserve_out, net_input, denominator)?;

        require!(output > 0, AmmError::InvalidAmount);
        require!(output >= minimum_output, AmmError::SlippageExceeded);

        Ok(SwapAmounts {
            net_input,
            fee,
            output,
        })
    }
}

fn checked_mul_div_floor(left: u64, right: u64, denominator: u64) -> Result<u64> {
    require!(denominator > 0, AmmError::Underflow);

    let result = u128::from(left)
        .checked_mul(u128::from(right))
        .ok_or_else(|| error!(AmmError::Overflow))?
        .checked_div(u128::from(denominator))
        .ok_or_else(|| error!(AmmError::Underflow))?;

    u64::try_from(result).map_err(|_| error!(AmmError::Overflow))
}

fn checked_mul_div_ceil(left: u64, right: u64, denominator: u64) -> Result<u64> {
    require!(denominator > 0, AmmError::Underflow);

    let product = u128::from(left)
        .checked_mul(u128::from(right))
        .ok_or_else(|| error!(AmmError::Overflow))?;
    let result = product
        .checked_add(u128::from(denominator) - 1)
        .ok_or_else(|| error!(AmmError::Overflow))?
        .checked_div(u128::from(denominator))
        .ok_or_else(|| error!(AmmError::Underflow))?;

    u64::try_from(result).map_err(|_| error!(AmmError::Overflow))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_rounds_required_token_amounts_up() {
        let amounts = ConstantProduct::deposit_amounts(10, 20, 6, 1).unwrap();

        assert_eq!(amounts, TokenAmounts { x: 2, y: 4 });
    }

    #[test]
    fn withdraw_rounds_proportional_token_amounts_down() {
        let amounts = ConstantProduct::withdraw_amounts(10, 20, 6, 1).unwrap();

        assert_eq!(amounts, TokenAmounts { x: 1, y: 3 });
    }

    #[test]
    fn swap_separates_fee_and_preserves_constant_product_invariant() {
        let amounts = ConstantProduct::swap(200_000_000, 300_000_000, 10_000_000, 30, 0).unwrap();
        let invariant_before = 200_000_000u128 * 300_000_000u128;
        let invariant_after =
            u128::from(200_000_000 + amounts.net_input) * u128::from(300_000_000 - amounts.output);

        assert_eq!(amounts.net_input, 9_970_000);
        assert_eq!(amounts.fee, 30_000);
        assert_eq!(amounts.output, 14_244_892);
        assert!(invariant_after >= invariant_before);
    }
}

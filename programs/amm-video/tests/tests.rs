use {
    anchor_lang::AccountDeserialize,
    anchor_spl::associated_token,
    litesvm::{types::TransactionResult, LiteSVM},
    litesvm_token::{
        get_spl_account,
        spl_token::state::{Account as TokenAccount, Mint},
        CreateAssociatedTokenAccount, CreateMint, MintTo,
    },
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

mod ix_handlers;
use ix_handlers::*;

const SEED: u64 = 123;
const FEE: u16 = 30;
const USER_BALANCE: u64 = 1_000_000_000;
const INITIAL_LP: u64 = 100_000_000;
const INITIAL_X: u64 = 200_000_000;
const INITIAL_Y: u64 = 300_000_000;

struct Fixture {
    svm: LiteSVM,
    payer: Keypair,
    authority: Keypair,
    treasury: Keypair,
    provider: Keypair,
    trader: Keypair,
    pool: PoolAccounts,
    provider_x: Pubkey,
    provider_y: Pubkey,
    provider_lp: Pubkey,
    trader_x: Pubkey,
    trader_y: Pubkey,
    trader_lp: Pubkey,
}

#[derive(Debug, Eq, PartialEq)]
struct PoolSnapshot {
    vault_x: u64,
    vault_y: u64,
    treasury_x: u64,
    treasury_y: u64,
    user_x: u64,
    user_y: u64,
    user_lp: Option<u64>,
    lp_supply: u64,
}

impl Fixture {
    fn new() -> Self {
        let payer = Keypair::new();
        let authority = Keypair::new();
        let treasury = Keypair::new();
        let provider = Keypair::new();
        let trader = Keypair::new();
        let mut svm = LiteSVM::new();
        let bytes = include_bytes!("../../../target/deploy/amm_video.so");

        svm.add_program(amm_video::id(), bytes).unwrap();
        for address in [
            payer.pubkey(),
            authority.pubkey(),
            treasury.pubkey(),
            provider.pubkey(),
            trader.pubkey(),
        ] {
            svm.airdrop(&address, 5_000_000_000).unwrap();
        }

        let mint_x = CreateMint::new(&mut svm, &payer)
            .decimals(6)
            .authority(&payer.pubkey())
            .send()
            .unwrap();
        let mint_y = CreateMint::new(&mut svm, &payer)
            .decimals(6)
            .authority(&payer.pubkey())
            .send()
            .unwrap();
        let pool = PoolAccounts::derive(SEED, mint_x, mint_y, treasury.pubkey());

        let provider_x = create_funded_token_account(
            &mut svm,
            &payer,
            &provider.pubkey(),
            &mint_x,
            USER_BALANCE,
        );
        let provider_y = create_funded_token_account(
            &mut svm,
            &payer,
            &provider.pubkey(),
            &mint_y,
            USER_BALANCE,
        );
        let trader_x =
            create_funded_token_account(&mut svm, &payer, &trader.pubkey(), &mint_x, USER_BALANCE);
        let trader_y =
            create_funded_token_account(&mut svm, &payer, &trader.pubkey(), &mint_y, USER_BALANCE);
        let provider_lp =
            associated_token::get_associated_token_address(&provider.pubkey(), &pool.mint_lp);
        let trader_lp =
            associated_token::get_associated_token_address(&trader.pubkey(), &pool.mint_lp);

        Self {
            svm,
            payer,
            authority,
            treasury,
            provider,
            trader,
            pool,
            provider_x,
            provider_y,
            provider_lp,
            trader_x,
            trader_y,
            trader_lp,
        }
    }

    fn initialize(&mut self) {
        self.initialize_with(Some(self.authority.pubkey()), FEE);
    }

    fn initialize_with(&mut self, authority: Option<Pubkey>, fee: u16) {
        let instruction =
            create_initialize_ix(self.payer.pubkey(), &self.pool, SEED, fee, authority);
        expect_success(send(
            &mut self.svm,
            &[instruction],
            &self.payer,
            &[&self.payer],
        ));
    }

    fn seed_liquidity(&mut self) {
        let instruction = create_deposit_ix(
            self.provider.pubkey(),
            &self.pool,
            self.provider_x,
            self.provider_y,
            self.provider_lp,
            INITIAL_LP,
            INITIAL_X,
            INITIAL_Y,
        );
        expect_success(send(
            &mut self.svm,
            &[instruction],
            &self.provider,
            &[&self.provider],
        ));
    }

    fn initialize_and_seed(&mut self) {
        self.initialize();
        self.seed_liquidity();
    }
}

fn create_funded_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    owner: &Pubkey,
    mint: &Pubkey,
    amount: u64,
) -> Pubkey {
    let account = CreateAssociatedTokenAccount::new(svm, payer, mint)
        .owner(owner)
        .send()
        .unwrap();
    MintTo::new(svm, payer, mint, &account, amount)
        .send()
        .unwrap();
    account
}

#[allow(clippy::result_large_err)]
fn send(
    svm: &mut LiteSVM,
    instructions: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(instructions, Some(&payer.pubkey()), &blockhash);
    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), signers).unwrap();
    svm.send_transaction(transaction)
}

fn expect_success(result: TransactionResult) {
    if let Err(error) = result {
        panic!(
            "transaction failed: {:?}\nlogs: {:#?}",
            error.err, error.meta.logs
        );
    }
}

fn assert_anchor_error(result: TransactionResult, expected_code: &str) {
    let error = result.expect_err("transaction should fail");
    let expected_log = format!("Error Code: {expected_code}");

    assert!(
        error
            .meta
            .logs
            .iter()
            .any(|log| log.contains(&expected_log)),
        "expected Anchor error {expected_code}, got {:?}\nlogs: {:#?}",
        error.err,
        error.meta.logs
    );
}

fn config_state(svm: &LiteSVM, config: &Pubkey) -> amm_video::state::Config {
    let account = svm.get_account(config).unwrap();
    let mut data = account.data.as_slice();

    amm_video::state::Config::try_deserialize(&mut data).unwrap()
}

fn token_account(svm: &LiteSVM, address: &Pubkey) -> TokenAccount {
    get_spl_account(svm, address).unwrap()
}

fn mint_state(svm: &LiteSVM, address: &Pubkey) -> Mint {
    get_spl_account(svm, address).unwrap()
}

fn token_amount(svm: &LiteSVM, address: &Pubkey) -> u64 {
    token_account(svm, address).amount
}

fn optional_token_amount(svm: &LiteSVM, address: &Pubkey) -> Option<u64> {
    svm.get_account(address)
        .map(|_| token_account(svm, address).amount)
}

fn pool_snapshot(
    svm: &LiteSVM,
    pool: &PoolAccounts,
    user_x: &Pubkey,
    user_y: &Pubkey,
    user_lp: &Pubkey,
) -> PoolSnapshot {
    PoolSnapshot {
        vault_x: token_amount(svm, &pool.vault_x),
        vault_y: token_amount(svm, &pool.vault_y),
        treasury_x: token_amount(svm, &pool.treasury_x),
        treasury_y: token_amount(svm, &pool.treasury_y),
        user_x: token_amount(svm, user_x),
        user_y: token_amount(svm, user_y),
        user_lp: optional_token_amount(svm, user_lp),
        lp_supply: mint_state(svm, &pool.mint_lp).supply,
    }
}

fn assert_pool_accounts_absent(svm: &LiteSVM, pool: &PoolAccounts) {
    for address in [
        pool.config,
        pool.mint_lp,
        pool.vault_x,
        pool.vault_y,
        pool.treasury_x,
        pool.treasury_y,
    ] {
        assert!(
            svm.get_account(&address).is_none(),
            "{address} should not exist"
        );
    }
}

#[test]
fn initialize_stores_config_and_creates_empty_pool_and_treasury_accounts() {
    let mut fixture = Fixture::new();
    fixture.initialize();

    let config = config_state(&fixture.svm, &fixture.pool.config);
    let config_account = fixture.svm.get_account(&fixture.pool.config).unwrap();
    let (_, config_bump) =
        Pubkey::find_program_address(&[b"config", SEED.to_le_bytes().as_ref()], &amm_video::id());
    let (_, lp_bump) =
        Pubkey::find_program_address(&[b"lp", fixture.pool.config.as_ref()], &amm_video::id());

    assert_eq!(config_account.owner, amm_video::id());
    assert_eq!(config.seed, SEED);
    assert_eq!(config.authority, Some(fixture.authority.pubkey()));
    assert_eq!(config.mint_x, fixture.pool.mint_x);
    assert_eq!(config.mint_y, fixture.pool.mint_y);
    assert_eq!(config.treasury, fixture.treasury.pubkey());
    assert_eq!(config.fee, FEE);
    assert!(!config.locked);
    assert_eq!(config.config_bump, config_bump);
    assert_eq!(config.lp_bump, lp_bump);

    let lp_mint = mint_state(&fixture.svm, &fixture.pool.mint_lp);
    assert_eq!(lp_mint.mint_authority.unwrap(), fixture.pool.config);
    assert_eq!(lp_mint.supply, 0);
    assert_eq!(lp_mint.decimals, 6);
    assert!(lp_mint.freeze_authority.is_none());

    for (address, mint, owner) in [
        (
            fixture.pool.vault_x,
            fixture.pool.mint_x,
            fixture.pool.config,
        ),
        (
            fixture.pool.vault_y,
            fixture.pool.mint_y,
            fixture.pool.config,
        ),
        (
            fixture.pool.treasury_x,
            fixture.pool.mint_x,
            fixture.treasury.pubkey(),
        ),
        (
            fixture.pool.treasury_y,
            fixture.pool.mint_y,
            fixture.treasury.pubkey(),
        ),
    ] {
        let account = token_account(&fixture.svm, &address);
        assert_eq!(account.mint, mint);
        assert_eq!(account.owner, owner);
        assert_eq!(account.amount, 0);
    }
}

#[test]
fn initialize_accepts_existing_canonical_treasury_accounts() {
    let mut fixture = Fixture::new();
    let treasury_x =
        CreateAssociatedTokenAccount::new(&mut fixture.svm, &fixture.payer, &fixture.pool.mint_x)
            .owner(&fixture.treasury.pubkey())
            .send()
            .unwrap();
    let treasury_y =
        CreateAssociatedTokenAccount::new(&mut fixture.svm, &fixture.payer, &fixture.pool.mint_y)
            .owner(&fixture.treasury.pubkey())
            .send()
            .unwrap();

    fixture.initialize();

    assert_eq!(treasury_x, fixture.pool.treasury_x);
    assert_eq!(treasury_y, fixture.pool.treasury_y);
    assert_eq!(token_amount(&fixture.svm, &treasury_x), 0);
    assert_eq!(token_amount(&fixture.svm, &treasury_y), 0);
    assert_eq!(
        config_state(&fixture.svm, &fixture.pool.config).treasury,
        fixture.treasury.pubkey()
    );
}

#[test]
fn initialize_rejects_100_percent_fee_without_creating_accounts() {
    let mut fixture = Fixture::new();
    let instruction = create_initialize_ix(
        fixture.payer.pubkey(),
        &fixture.pool,
        SEED,
        10_000,
        Some(fixture.authority.pubkey()),
    );

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.payer,
            &[&fixture.payer],
        ),
        "FeePercentErr",
    );
    assert_pool_accounts_absent(&fixture.svm, &fixture.pool);
}

#[test]
fn initialize_rejects_wrong_mint_precision_without_creating_accounts() {
    let mut fixture = Fixture::new();
    let mint = CreateMint::new(&mut fixture.svm, &fixture.payer)
        .decimals(5)
        .authority(&fixture.payer.pubkey())
        .send()
        .unwrap();
    let pool = PoolAccounts::derive(
        SEED + 1,
        mint,
        fixture.pool.mint_y,
        fixture.treasury.pubkey(),
    );
    let instruction = create_initialize_ix(
        fixture.payer.pubkey(),
        &pool,
        SEED + 1,
        FEE,
        Some(fixture.authority.pubkey()),
    );

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.payer,
            &[&fixture.payer],
        ),
        "InvalidPrecision",
    );
    assert_pool_accounts_absent(&fixture.svm, &pool);
}

#[test]
fn initialize_rejects_identical_mints_without_creating_accounts() {
    let mut fixture = Fixture::new();
    let pool = PoolAccounts::derive(
        SEED + 2,
        fixture.pool.mint_x,
        fixture.pool.mint_x,
        fixture.treasury.pubkey(),
    );
    let instruction = create_initialize_ix(
        fixture.payer.pubkey(),
        &pool,
        SEED + 2,
        FEE,
        Some(fixture.authority.pubkey()),
    );

    assert!(send(
        &mut fixture.svm,
        &[instruction],
        &fixture.payer,
        &[&fixture.payer],
    )
    .is_err());
    assert_pool_accounts_absent(&fixture.svm, &pool);
}

#[test]
fn initial_deposit_transfers_max_tokens_and_mints_requested_lp() {
    let mut fixture = Fixture::new();
    fixture.initialize();
    fixture.seed_liquidity();

    assert_eq!(
        token_amount(&fixture.svm, &fixture.provider_x),
        USER_BALANCE - INITIAL_X
    );
    assert_eq!(
        token_amount(&fixture.svm, &fixture.provider_y),
        USER_BALANCE - INITIAL_Y
    );
    assert_eq!(token_amount(&fixture.svm, &fixture.pool.vault_x), INITIAL_X);
    assert_eq!(token_amount(&fixture.svm, &fixture.pool.vault_y), INITIAL_Y);
    assert_eq!(token_amount(&fixture.svm, &fixture.provider_lp), INITIAL_LP);
    assert_eq!(
        mint_state(&fixture.svm, &fixture.pool.mint_lp).supply,
        INITIAL_LP
    );
    assert_eq!(token_amount(&fixture.svm, &fixture.pool.treasury_x), 0);
    assert_eq!(token_amount(&fixture.svm, &fixture.pool.treasury_y), 0);
}

#[test]
fn subsequent_deposit_uses_current_reserve_ratio() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let lp_amount = 50_000_000;
    let required_x = 100_000_000;
    let required_y = 150_000_000;
    let instruction = create_deposit_ix(
        fixture.trader.pubkey(),
        &fixture.pool,
        fixture.trader_x,
        fixture.trader_y,
        fixture.trader_lp,
        lp_amount,
        required_x,
        required_y,
    );

    expect_success(send(
        &mut fixture.svm,
        &[instruction],
        &fixture.trader,
        &[&fixture.trader],
    ));

    assert_eq!(
        token_amount(&fixture.svm, &fixture.trader_x),
        USER_BALANCE - required_x
    );
    assert_eq!(
        token_amount(&fixture.svm, &fixture.trader_y),
        USER_BALANCE - required_y
    );
    assert_eq!(token_amount(&fixture.svm, &fixture.trader_lp), lp_amount);
    assert_eq!(
        token_amount(&fixture.svm, &fixture.pool.vault_x),
        INITIAL_X + required_x
    );
    assert_eq!(
        token_amount(&fixture.svm, &fixture.pool.vault_y),
        INITIAL_Y + required_y
    );
    assert_eq!(
        mint_state(&fixture.svm, &fixture.pool.mint_lp).supply,
        INITIAL_LP + lp_amount
    );
}

#[test]
fn deposit_above_maximum_fails_without_changing_token_state() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let before = pool_snapshot(
        &fixture.svm,
        &fixture.pool,
        &fixture.trader_x,
        &fixture.trader_y,
        &fixture.trader_lp,
    );
    let instruction = create_deposit_ix(
        fixture.trader.pubkey(),
        &fixture.pool,
        fixture.trader_x,
        fixture.trader_y,
        fixture.trader_lp,
        50_000_000,
        99_999_999,
        150_000_000,
    );

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.trader,
            &[&fixture.trader],
        ),
        "SlippageExceeded",
    );
    assert_eq!(
        pool_snapshot(
            &fixture.svm,
            &fixture.pool,
            &fixture.trader_x,
            &fixture.trader_y,
            &fixture.trader_lp,
        ),
        before
    );
}

#[test]
fn zero_deposit_is_rejected_without_mutating_the_pool() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let before = pool_snapshot(
        &fixture.svm,
        &fixture.pool,
        &fixture.provider_x,
        &fixture.provider_y,
        &fixture.provider_lp,
    );
    let instruction = create_deposit_ix(
        fixture.provider.pubkey(),
        &fixture.pool,
        fixture.provider_x,
        fixture.provider_y,
        fixture.provider_lp,
        0,
        0,
        0,
    );

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.provider,
            &[&fixture.provider],
        ),
        "InvalidAmount",
    );
    assert_eq!(
        pool_snapshot(
            &fixture.svm,
            &fixture.pool,
            &fixture.provider_x,
            &fixture.provider_y,
            &fixture.provider_lp,
        ),
        before
    );
}

#[test]
fn withdraw_burns_lp_and_returns_proportional_reserves() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let lp_amount = 25_000_000;
    let expected_x = 50_000_000;
    let expected_y = 75_000_000;
    let instruction = create_withdraw_ix(
        fixture.provider.pubkey(),
        &fixture.pool,
        fixture.provider_x,
        fixture.provider_y,
        fixture.provider_lp,
        lp_amount,
        expected_x,
        expected_y,
    );

    expect_success(send(
        &mut fixture.svm,
        &[instruction],
        &fixture.provider,
        &[&fixture.provider],
    ));

    assert_eq!(
        token_amount(&fixture.svm, &fixture.provider_x),
        USER_BALANCE - INITIAL_X + expected_x
    );
    assert_eq!(
        token_amount(&fixture.svm, &fixture.provider_y),
        USER_BALANCE - INITIAL_Y + expected_y
    );
    assert_eq!(
        token_amount(&fixture.svm, &fixture.provider_lp),
        INITIAL_LP - lp_amount
    );
    assert_eq!(
        token_amount(&fixture.svm, &fixture.pool.vault_x),
        INITIAL_X - expected_x
    );
    assert_eq!(
        token_amount(&fixture.svm, &fixture.pool.vault_y),
        INITIAL_Y - expected_y
    );
    assert_eq!(
        mint_state(&fixture.svm, &fixture.pool.mint_lp).supply,
        INITIAL_LP - lp_amount
    );
}

#[test]
fn withdraw_below_minimum_fails_without_changing_token_state() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let before = pool_snapshot(
        &fixture.svm,
        &fixture.pool,
        &fixture.provider_x,
        &fixture.provider_y,
        &fixture.provider_lp,
    );
    let instruction = create_withdraw_ix(
        fixture.provider.pubkey(),
        &fixture.pool,
        fixture.provider_x,
        fixture.provider_y,
        fixture.provider_lp,
        25_000_000,
        50_000_001,
        75_000_000,
    );

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.provider,
            &[&fixture.provider],
        ),
        "SlippageExceeded",
    );
    assert_eq!(
        pool_snapshot(
            &fixture.svm,
            &fixture.pool,
            &fixture.provider_x,
            &fixture.provider_y,
            &fixture.provider_lp,
        ),
        before
    );
}

#[test]
fn withdraw_above_user_lp_balance_fails_without_changing_token_state() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let before = pool_snapshot(
        &fixture.svm,
        &fixture.pool,
        &fixture.provider_x,
        &fixture.provider_y,
        &fixture.provider_lp,
    );
    let instruction = create_withdraw_ix(
        fixture.provider.pubkey(),
        &fixture.pool,
        fixture.provider_x,
        fixture.provider_y,
        fixture.provider_lp,
        INITIAL_LP + 1,
        0,
        0,
    );

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.provider,
            &[&fixture.provider],
        ),
        "InsufficientBalance",
    );
    assert_eq!(
        pool_snapshot(
            &fixture.svm,
            &fixture.pool,
            &fixture.provider_x,
            &fixture.provider_y,
            &fixture.provider_lp,
        ),
        before
    );
}

#[test]
fn x_to_y_swap_sends_net_input_to_pool_and_fee_to_treasury() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let gross_input = 10_000_000;
    let net_input = 9_970_000;
    let fee = 30_000;
    let expected_output = 14_244_892;
    let instruction = create_swap_ix(
        fixture.trader.pubkey(),
        &fixture.pool,
        fixture.trader_x,
        fixture.trader_y,
        true,
        gross_input,
        expected_output,
    );

    expect_success(send(
        &mut fixture.svm,
        &[instruction],
        &fixture.trader,
        &[&fixture.trader],
    ));

    let vault_x = token_amount(&fixture.svm, &fixture.pool.vault_x);
    let vault_y = token_amount(&fixture.svm, &fixture.pool.vault_y);
    assert_eq!(
        token_amount(&fixture.svm, &fixture.trader_x),
        USER_BALANCE - gross_input
    );
    assert_eq!(
        token_amount(&fixture.svm, &fixture.trader_y),
        USER_BALANCE + expected_output
    );
    assert_eq!(vault_x, INITIAL_X + net_input);
    assert_eq!(vault_y, INITIAL_Y - expected_output);
    assert_eq!(token_amount(&fixture.svm, &fixture.pool.treasury_x), fee);
    assert_eq!(token_amount(&fixture.svm, &fixture.pool.treasury_y), 0);
    assert_eq!(
        mint_state(&fixture.svm, &fixture.pool.mint_lp).supply,
        INITIAL_LP
    );
    assert!(
        u128::from(vault_x) * u128::from(vault_y) >= u128::from(INITIAL_X) * u128::from(INITIAL_Y)
    );
}

#[test]
fn y_to_x_swap_sends_net_input_to_pool_and_fee_to_treasury() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let gross_input = 10_000_000;
    let net_input = 9_970_000;
    let fee = 30_000;
    let expected_output = 6_432_880;
    let instruction = create_swap_ix(
        fixture.trader.pubkey(),
        &fixture.pool,
        fixture.trader_x,
        fixture.trader_y,
        false,
        gross_input,
        expected_output,
    );

    expect_success(send(
        &mut fixture.svm,
        &[instruction],
        &fixture.trader,
        &[&fixture.trader],
    ));

    let vault_x = token_amount(&fixture.svm, &fixture.pool.vault_x);
    let vault_y = token_amount(&fixture.svm, &fixture.pool.vault_y);
    assert_eq!(
        token_amount(&fixture.svm, &fixture.trader_x),
        USER_BALANCE + expected_output
    );
    assert_eq!(
        token_amount(&fixture.svm, &fixture.trader_y),
        USER_BALANCE - gross_input
    );
    assert_eq!(vault_x, INITIAL_X - expected_output);
    assert_eq!(vault_y, INITIAL_Y + net_input);
    assert_eq!(token_amount(&fixture.svm, &fixture.pool.treasury_x), 0);
    assert_eq!(token_amount(&fixture.svm, &fixture.pool.treasury_y), fee);
    assert_eq!(
        mint_state(&fixture.svm, &fixture.pool.mint_lp).supply,
        INITIAL_LP
    );
    assert!(
        u128::from(vault_x) * u128::from(vault_y) >= u128::from(INITIAL_X) * u128::from(INITIAL_Y)
    );
}

#[test]
fn swap_rejects_spoofed_treasury_without_moving_tokens() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let attacker = Keypair::new();
    fixture
        .svm
        .airdrop(&attacker.pubkey(), 5_000_000_000)
        .unwrap();
    let spoofed_pool = PoolAccounts::derive(
        SEED,
        fixture.pool.mint_x,
        fixture.pool.mint_y,
        attacker.pubkey(),
    );
    let attacker_x =
        CreateAssociatedTokenAccount::new(&mut fixture.svm, &fixture.payer, &fixture.pool.mint_x)
            .owner(&attacker.pubkey())
            .send()
            .unwrap();
    let attacker_y =
        CreateAssociatedTokenAccount::new(&mut fixture.svm, &fixture.payer, &fixture.pool.mint_y)
            .owner(&attacker.pubkey())
            .send()
            .unwrap();
    assert_eq!(attacker_x, spoofed_pool.treasury_x);
    assert_eq!(attacker_y, spoofed_pool.treasury_y);
    let before = pool_snapshot(
        &fixture.svm,
        &fixture.pool,
        &fixture.trader_x,
        &fixture.trader_y,
        &fixture.trader_lp,
    );
    let instruction = create_swap_ix(
        fixture.trader.pubkey(),
        &spoofed_pool,
        fixture.trader_x,
        fixture.trader_y,
        true,
        10_000_000,
        0,
    );

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.trader,
            &[&fixture.trader],
        ),
        "ConstraintHasOne",
    );
    assert_eq!(
        pool_snapshot(
            &fixture.svm,
            &fixture.pool,
            &fixture.trader_x,
            &fixture.trader_y,
            &fixture.trader_lp,
        ),
        before
    );
}

#[test]
fn swap_below_minimum_output_fails_without_moving_tokens() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let before = pool_snapshot(
        &fixture.svm,
        &fixture.pool,
        &fixture.trader_x,
        &fixture.trader_y,
        &fixture.trader_lp,
    );
    let instruction = create_swap_ix(
        fixture.trader.pubkey(),
        &fixture.pool,
        fixture.trader_x,
        fixture.trader_y,
        true,
        10_000_000,
        14_244_893,
    );

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.trader,
            &[&fixture.trader],
        ),
        "SlippageExceeded",
    );
    assert_eq!(
        pool_snapshot(
            &fixture.svm,
            &fixture.pool,
            &fixture.trader_x,
            &fixture.trader_y,
            &fixture.trader_lp,
        ),
        before
    );
}

#[test]
fn swap_rejects_empty_pool_without_moving_tokens() {
    let mut fixture = Fixture::new();
    fixture.initialize();
    let before = pool_snapshot(
        &fixture.svm,
        &fixture.pool,
        &fixture.trader_x,
        &fixture.trader_y,
        &fixture.trader_lp,
    );
    let instruction = create_swap_ix(
        fixture.trader.pubkey(),
        &fixture.pool,
        fixture.trader_x,
        fixture.trader_y,
        true,
        10_000_000,
        0,
    );

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.trader,
            &[&fixture.trader],
        ),
        "NoLiquidityInPool",
    );
    assert_eq!(
        pool_snapshot(
            &fixture.svm,
            &fixture.pool,
            &fixture.trader_x,
            &fixture.trader_y,
            &fixture.trader_lp,
        ),
        before
    );
}

#[test]
fn set_locked_blocks_deposit_withdraw_and_swap_until_unlocked() {
    let mut fixture = Fixture::new();
    fixture.initialize_and_seed();
    let lock = create_set_locked_ix(fixture.authority.pubkey(), fixture.pool.config, true);
    expect_success(send(
        &mut fixture.svm,
        &[lock],
        &fixture.authority,
        &[&fixture.authority],
    ));
    assert!(config_state(&fixture.svm, &fixture.pool.config).locked);

    let before = pool_snapshot(
        &fixture.svm,
        &fixture.pool,
        &fixture.provider_x,
        &fixture.provider_y,
        &fixture.provider_lp,
    );
    let deposit = create_deposit_ix(
        fixture.provider.pubkey(),
        &fixture.pool,
        fixture.provider_x,
        fixture.provider_y,
        fixture.provider_lp,
        1_000_000,
        2_000_000,
        3_000_000,
    );
    let withdraw = create_withdraw_ix(
        fixture.provider.pubkey(),
        &fixture.pool,
        fixture.provider_x,
        fixture.provider_y,
        fixture.provider_lp,
        1_000_000,
        0,
        0,
    );
    let swap = create_swap_ix(
        fixture.provider.pubkey(),
        &fixture.pool,
        fixture.provider_x,
        fixture.provider_y,
        true,
        1_000_000,
        0,
    );

    for instruction in [deposit, withdraw, swap] {
        assert_anchor_error(
            send(
                &mut fixture.svm,
                &[instruction],
                &fixture.provider,
                &[&fixture.provider],
            ),
            "PoolLocked",
        );
        assert_eq!(
            pool_snapshot(
                &fixture.svm,
                &fixture.pool,
                &fixture.provider_x,
                &fixture.provider_y,
                &fixture.provider_lp,
            ),
            before
        );
    }

    let unlock = create_set_locked_ix(fixture.authority.pubkey(), fixture.pool.config, false);
    expect_success(send(
        &mut fixture.svm,
        &[unlock],
        &fixture.authority,
        &[&fixture.authority],
    ));
    assert!(!config_state(&fixture.svm, &fixture.pool.config).locked);

    let swap = create_swap_ix(
        fixture.provider.pubkey(),
        &fixture.pool,
        fixture.provider_x,
        fixture.provider_y,
        true,
        1_000_000,
        0,
    );
    expect_success(send(
        &mut fixture.svm,
        &[swap],
        &fixture.provider,
        &[&fixture.provider],
    ));
    assert_eq!(token_amount(&fixture.svm, &fixture.pool.treasury_x), 3_000);
    assert_eq!(
        token_amount(&fixture.svm, &fixture.provider_x),
        before.user_x - 1_000_000
    );
}

#[test]
fn set_locked_rejects_unauthorized_signer_without_changing_config() {
    let mut fixture = Fixture::new();
    fixture.initialize();
    let before = config_state(&fixture.svm, &fixture.pool.config);
    let instruction = create_set_locked_ix(fixture.trader.pubkey(), fixture.pool.config, true);

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.trader,
            &[&fixture.trader],
        ),
        "InvalidAuthority",
    );
    let after = config_state(&fixture.svm, &fixture.pool.config);
    assert_eq!(after.seed, before.seed);
    assert_eq!(after.authority, before.authority);
    assert_eq!(after.mint_x, before.mint_x);
    assert_eq!(after.mint_y, before.mint_y);
    assert_eq!(after.treasury, before.treasury);
    assert_eq!(after.fee, before.fee);
    assert_eq!(after.locked, before.locked);
}

#[test]
fn set_locked_rejects_pool_without_configured_authority() {
    let mut fixture = Fixture::new();
    fixture.initialize_with(None, FEE);
    let instruction = create_set_locked_ix(fixture.authority.pubkey(), fixture.pool.config, true);

    assert_anchor_error(
        send(
            &mut fixture.svm,
            &[instruction],
            &fixture.authority,
            &[&fixture.authority],
        ),
        "NoAuthoritySet",
    );
    assert!(!config_state(&fixture.svm, &fixture.pool.config).locked);
}

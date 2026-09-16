use super::*;
use super::local::{Bin, PoolMath, Tick, VariableFee};
use crate::{dex::DexKind, rpc::AccountData};
use rust_decimal::Decimal;

fn cfg() -> ArbitrageConfig {
    ArbitrageConfig {
        enabled: true, min_input_usdc: Decimal::new(1, 6), max_input_usdc: Decimal::new(1000, 6),
        min_profit_usdc: Decimal::ZERO, notification_profit_change_usdc: Decimal::new(10, 6),
        input_step_raw: 1, coarse_points: 9, max_route_quotes: 100,
        max_price_impact_bps: 9999, max_slot_difference: 2,
        costs: FixedCosts { base_fee_usdc: Decimal::ZERO, priority_fee_usdc: Decimal::ZERO,
            jito_tip_usdc: Decimal::ZERO, flashloan_fee_usdc: Decimal::ZERO, slippage_buffer_usdc: Decimal::ZERO },
        accounts: vec![],
    }
}

fn ray(sol: u64, usdc: u64) -> LocalPool {
    LocalPool { dex: DexKind::Raydium, address: "ray".into(),
        math: PoolMath::Raydium { sol, usdc, fee_n: 0, fee_d: 10_000 } }
}

fn record() -> CandidateEvaluation {
    CandidateEvaluation { observed_at: Utc::now().to_rfc3339(), route_key: "test-route".into(),
        buy_dex: "Raydium".into(), sell_dex: "Orca".into(), buy_pool: "buy".into(), sell_pool: "sell".into(),
        state: "detected".into(), input_raw: Some(10), output_raw: Some(110), expected_profit_raw: Some(100),
        min_profit_raw: 0, fixed_costs: cfg().costs, fixed_cost_raw: 0, min_slot: Some(100), max_slot: Some(102),
        trials: vec![], error: None }
}

#[test]
fn usdc_configuration_never_silently_rounds() {
    assert_eq!(usdc_raw(Decimal::new(1234567, 6)).unwrap(), 1234567);
    assert!(usdc_raw(Decimal::new(1, 7)).is_err());
    assert!(usdc_raw(Decimal::NEGATIVE_ONE).is_err());
}

#[test]
fn every_fixed_cost_is_subtracted() {
    let costs = FixedCosts { base_fee_usdc: Decimal::new(1, 6), priority_fee_usdc: Decimal::new(2, 6),
        jito_tip_usdc: Decimal::new(4, 6), flashloan_fee_usdc: Decimal::new(8, 6), slippage_buffer_usdc: Decimal::new(16, 6) };
    assert_eq!(costs.total_raw().unwrap(), 31);
}

#[test]
fn raydium_exact_in_rounds_fee_up_and_output_down() {
    let mut p = ray(1000, 1000);
    p.math = PoolMath::Raydium { sol: 1000, usdc: 1000, fee_n: 25, fee_d: 10_000 };
    let q = p.quote(100, true).unwrap();
    assert_eq!(q.fee_raw, 1);
    assert_eq!(q.output_raw, 90); // floor(99 * 1000 / 1099)
    assert_eq!(p.quote(100, false).unwrap().output_raw, 90);
    assert!(q.complete);
}

#[test]
fn large_intermediate_products_do_not_wrap() {
    assert_eq!(math::mul_div(u128::MAX, u128::MAX, u128::MAX, false).unwrap(), u128::MAX);
    assert!(math::mul_div(u128::MAX, u128::MAX, 1, false).is_err());
    assert!(math::mul_div(1, 1, 0, false).is_err());
}

#[test]
fn tick_reference_values_match_integer_definition() {
    assert_eq!(math::tick_sqrt(0).unwrap(), math::Q64);
    assert_eq!(math::tick_sqrt(1).unwrap(), 18447666387855959850);
    assert_eq!(math::tick_sqrt(-1).unwrap(), 18445821805675392311);
    assert_eq!(math::tick_sqrt(443636).unwrap(), 79226673515401279992447579055);
    assert_eq!(math::tick_sqrt(-443636).unwrap(), 4295048016);
    assert!(math::tick_sqrt(443637).is_err());
}

fn orca(sol_is_a: bool) -> LocalPool {
    LocalPool { dex: DexKind::Orca, address: "orca".into(),
        math: PoolMath::Orca { sol_is_a, sqrt: math::Q64, liquidity: 1_000_000,
            current_tick: 0, fee: 0, lower_tick: -88, upper_tick: 88, ticks: vec![] } }
}

#[test]
fn whirlpool_directions_and_mint_inversion_use_raw_units() {
    for buy in [true, false] {
        let q = orca(true).quote(1000, buy).unwrap();
        assert!(q.complete);
        assert_eq!(q.output_raw, 999);
        assert_eq!(q.output_raw, orca(false).quote(1000, !buy).unwrap().output_raw);
    }
}

#[test]
fn whirlpool_crossing_changes_liquidity_and_stops_at_missing_array() {
    let mut p = orca(true);
    if let PoolMath::Orca { ticks, .. } = &mut p.math {
        ticks.push(Tick { index: 10, liquidity_net: 1_000_000 });
    }
    let crossed = p.quote(2000, true).unwrap();
    assert!(crossed.complete);
    assert!(!crossed.boundary_inputs.is_empty());
    assert!(crossed.output_raw > orca(true).quote(2000, true).unwrap().output_raw);
    let exhausted = p.quote(1_000_000, true).unwrap();
    assert!(!exhausted.complete);
    assert!(exhausted.consumed_raw < exhausted.input_raw);
}

fn dlmm() -> LocalPool {
    LocalPool { dex: DexKind::MeteoraDlmm, address: "dlmm".into(),
        math: PoolMath::Meteora { sol_is_x: true, active_id: 0,
            bins: vec![Bin { id: -1, x: 200, y: 200, price: math::Q64 },
                Bin { id: 0, x: 100, y: 100, price: math::Q64 },
                Bin { id: 1, x: 200, y: 200, price: math::Q64 }],
            fee: VariableFee { base: 0, control: 0, bin_step: 1, reference: 0, index_reference: 0, max_accumulator: 100000 } } }
}

#[test]
fn dlmm_crosses_bins_in_both_directions_and_rejects_partial_fill() {
    for buy in [true, false] {
        let q = dlmm().quote(150, buy).unwrap();
        assert_eq!(q.output_raw, 150);
        assert_eq!(q.boundary_inputs, vec![100]);
        assert!(q.complete);
        let q = dlmm().quote(400, buy).unwrap();
        assert_eq!(q.consumed_raw, 300);
        assert!(!q.complete);
        assert!(!dlmm().within_impact(&q, buy, 9999).unwrap());
    }
}

#[test]
fn dlmm_variable_fee_increases_after_bin_crossing() {
    let mut p = dlmm();
    if let PoolMath::Meteora { fee, .. } = &mut p.math {
        fee.control = 100_000;
        fee.bin_step = 100;
    }
    assert_eq!(p.quote(100, true).unwrap().fee_raw, 0);
    assert!(p.quote(150, true).unwrap().fee_raw > 0);
}

#[test]
fn strict_threshold_and_costs_control_candidate_state() {
    let config = cfg();
    let buy = ray(10000, 10000);
    let sell = ray(10000, 12000);
    let result = evaluate(record(), &buy, &sell, &config).unwrap();
    assert_eq!(result.state, "detected");
    let mut equal = record();
    equal.min_profit_raw = result.expected_profit_raw.unwrap() as u64;
    assert_eq!(evaluate(equal, &buy, &sell, &config).unwrap().state, "unprofitable");
    let mut costly = record();
    costly.fixed_cost_raw = 1000;
    assert_eq!(evaluate(costly, &buy, &sell, &config).unwrap().state, "unprofitable");
    assert_eq!(evaluate(record(), &sell, &buy, &config).unwrap().state, "no_spread");
}

#[test]
fn search_observes_budget_endpoints_and_selects_feasible_smaller_input() {
    let config = cfg();
    let trials = search_route(&ray(10000, 10000), &dlmm(), &config, 0).unwrap();
    assert!(trials.len() <= config.max_route_quotes);
    assert_eq!(trials.first().unwrap().input_raw, 1);
    assert_eq!(trials.last().unwrap().input_raw, 1000);
    assert!(trials.iter().any(|t| t.profit_raw.is_some()));
    assert!(trials.iter().any(|t| t.error.is_some()));
}

#[test]
fn snapshot_uses_all_account_slots_including_vaults() {
    let account = |slot| AccountData { address: "a".into(), owner: "o".into(), lamports: 0, data: vec![], slot };
    assert_eq!(snapshot::slot_range(&[account(100), account(102)], 2).unwrap(), (100, 102));
    assert!(snapshot::slot_range(&[account(99), account(102)], 2).is_err());
    assert!(snapshot::slot_range(&[], 2).is_err());
}

#[test]
fn notification_tracks_cumulative_change_and_candidate_disappearance() {
    let mut c = record();
    assert!(c.should_notify(None, 10));
    let previous = NotificationState { state: "detected".into(), profit_raw: Some(100) };
    c.expected_profit_raw = Some(109);
    assert!(!c.should_notify(Some(&previous), 10));
    c.expected_profit_raw = Some(110);
    assert!(c.should_notify(Some(&previous), 10));
    c.state = "quote_failed".into();
    assert!(c.should_notify(Some(&previous), 10));
    assert!(!c.should_notify(None, 10));
}

#[test]
fn sqlite_migration_is_idempotent_and_notification_state_survives_readback() {
    let storage = Storage::open(":memory:").unwrap();
    storage.init_schema().unwrap();
    storage.init_schema().unwrap();
    let mut c = record();
    c.input_raw = Some(u64::MAX);
    c.expected_profit_raw = Some(-i128::from(u64::MAX));
    storage.insert_candidate_evaluation(&c).unwrap();
    assert!(storage.candidate_notification_state(&c.route_key).unwrap().is_none());
    storage.save_candidate_notification_state(&c).unwrap();
    let restored = storage.candidate_notification_state(&c.route_key).unwrap().unwrap();
    assert_eq!(restored.profit_raw, c.expected_profit_raw);
    assert!(!c.should_notify(Some(&restored), 10));
}

// DEXの最低限のバイナリレイアウトを使い、mathだけでなくsnapshotからの接続も確認する。
fn snapshot_fixture() -> (Vec<crate::config::PoolConfig>, ArbitrageConfig, std::collections::HashMap<String, AccountData>) {
    use crate::config::PoolConfig;
    let sol = "So11111111111111111111111111111111111111112";
    let usdc = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    let token = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
    let address = |n| bs58::encode([n; 32]).into_string();
    let mut config = cfg();
    let mut pools = Vec::new();
    let mut accounts = std::collections::HashMap::new();
    let mut add = |address: String, owner: &str, data: Vec<u8>| {
        accounts.insert(address.clone(), AccountData { address, owner: owner.into(), lamports: 0, data, slot: 100 });
    };
    for (mint, decimals) in [(sol, 9), (usdc, 6)] {
        let mut d = vec![0u8; 82]; d[44] = decimals; d[45] = 1;
        add(mint.into(), token, d);
    }
    for (n, dex) in [(1u8, DexKind::Raydium), (2u8, DexKind::Orca), (3u8, DexKind::MeteoraDlmm)] {
        let owner = address(n + 50);
        let pool_address = address(n);
        let vault_a = address(n + 10);
        let vault_b = address(n + 20);
        for (v, m) in [(&vault_a, sol), (&vault_b, usdc)] {
            let mut d = vec![0u8; 165];
            d[..32].copy_from_slice(&bs58::decode(m).into_vec().unwrap());
            d[64..72].copy_from_slice(&1_000_000_000_000u64.to_le_bytes());
            d[108] = 1;
            add(v.clone(), token, d);
        }
        let mut arrays = Vec::new();
        let mut d = vec![0u8; match dex { DexKind::Raydium => 752, DexKind::Orca => 653, DexKind::MeteoraDlmm => 904 }];
        let mut put_key = |o: usize, value: &str| d[o..o + 32].copy_from_slice(&bs58::decode(value).into_vec().unwrap());
        match dex {
            DexKind::Raydium => {
                put_key(336, &vault_a); put_key(368, &vault_b); put_key(400, sol); put_key(432, usdc);
                d[0..8].copy_from_slice(&6u64.to_le_bytes());
                d[32..40].copy_from_slice(&9u64.to_le_bytes());
                d[40..48].copy_from_slice(&6u64.to_le_bytes());
                d[120..128].copy_from_slice(&25u64.to_le_bytes());
                d[128..136].copy_from_slice(&10_000u64.to_le_bytes());
            }
            DexKind::Orca => {
                put_key(101, sol); put_key(133, &vault_a); put_key(181, usdc); put_key(213, &vault_b);
                d[..8].copy_from_slice(&[63, 149, 209, 12, 225, 128, 99, 9]);
                d[41..43].copy_from_slice(&1u16.to_le_bytes());
                d[43..45].copy_from_slice(&1u16.to_le_bytes());
                d[49..65].copy_from_slice(&1_000_000u128.to_le_bytes());
                d[65..81].copy_from_slice(&math::Q64.to_le_bytes());
                for (id, start) in [(70u8, -88i32), (71u8, 0i32)] {
                    let mut a = vec![0; 9988];
                    a[..8].copy_from_slice(&[69, 97, 189, 190, 110, 7, 66, 187]);
                    a[8..12].copy_from_slice(&start.to_le_bytes());
                    a[9956..9988].copy_from_slice(&bs58::decode(&pool_address).into_vec().unwrap());
                    arrays.push(address(id));
                    add(address(id), &owner, a);
                }
            }
            DexKind::MeteoraDlmm => {
                put_key(88, sol); put_key(120, usdc); put_key(152, &vault_a); put_key(184, &vault_b);
                d[..8].copy_from_slice(&[33, 11, 49, 98, 181, 101, 177, 13]);
                d[80..82].copy_from_slice(&1u16.to_le_bytes());
                d[20..24].copy_from_slice(&100_000u32.to_le_bytes());
                let mut a = vec![0; 10136];
                a[..8].copy_from_slice(&[92, 142, 92, 220, 5, 148, 70, 181]);
                a[24..56].copy_from_slice(&bs58::decode(&pool_address).into_vec().unwrap());
                a[56..64].copy_from_slice(&1_000_000u64.to_le_bytes());
                a[64..72].copy_from_slice(&1_000_000u64.to_le_bytes());
                a[72..88].copy_from_slice(&math::Q64.to_le_bytes());
                arrays.push(address(72));
                add(address(72), &owner, a);
            }
        }
        add(pool_address.clone(), &owner, d);
        config.accounts.push(QuoteAccountsConfig { pool_address: pool_address.clone(), program_id: owner, arrays });
        pools.push(PoolConfig { dex, pair: "SOL/USDC".into(), pool_address: pool_address.clone(),
            lb_pair_address: Some(pool_address), base_mint: sol.into(), quote_mint: usdc.into(),
            price_orientation: Some("usdc_per_sol".into()), auto_discovery: Some(false), enabled: true });
    }
    let mut clock = vec![0; 40]; clock[32..40].copy_from_slice(&100i64.to_le_bytes());
    add("SysvarC1ock11111111111111111111111111111111".into(), "sysvar", clock);
    (pools, config, accounts)
}

#[test]
fn snapshot_decodes_all_three_dexes_and_checks_pnl_and_owners() {
    let (pools, config, mut accounts) = snapshot_fixture();
    config.validate(&pools).unwrap();
    for pool in &pools {
        let decoded = snapshot::decode(pool, &config, &accounts, 100).unwrap();
        for buy in [true, false] {
            let q = decoded.quote(1000, buy).unwrap();
            assert!(q.complete && q.output_raw > 0);
        }
    }
    let pool = &pools[0];
    let before = snapshot::decode(pool, &config, &accounts, 100).unwrap().quote(1000, true).unwrap().output_raw;
    accounts.get_mut(&pool.account_address()).unwrap().data[192..200]
        .copy_from_slice(&500_000_000_000u64.to_le_bytes());
    let after = snapshot::decode(pool, &config, &accounts, 100).unwrap().quote(1000, true).unwrap().output_raw;
    assert!(after < before);
    accounts.get_mut(&pool.account_address()).unwrap().owner = "wrong-owner".into();
    assert!(snapshot::decode(pool, &config, &accounts, 100).is_err());
}

#[test]
fn configuration_rejects_missing_arrays_duplicates_and_zero_search_budget() {
    let (pools, mut config, _) = snapshot_fixture();
    config.max_route_quotes = 0;
    assert!(config.validate(&pools).is_err());
    config.max_route_quotes = 100;
    config.accounts[1].arrays.clear();
    assert!(config.validate(&pools).is_err());
    config.accounts[1] = config.accounts[0].clone();
    assert!(config.validate(&pools).is_err());
}

#[tokio::test]
async fn snapshot_refetches_all_accounts_once_after_inconsistent_slot() {
    use base64::Engine;
    use std::io::{BufRead, BufReader, Read, Write};
    let (pools, config, accounts) = snapshot_fixture();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for slot in [99u64, 0, 100, 100] {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0usize;
            loop {
                let mut line = String::new(); reader.read_line(&mut line).unwrap();
                if line == "\r\n" { break; }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length]; reader.read_exact(&mut body).unwrap();
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let values: Vec<_> = request["params"][0].as_array().unwrap().iter().map(|a| {
                let account = accounts.get(a.as_str().unwrap()).unwrap();
                serde_json::json!({ "owner": account.owner, "lamports": 0,
                    "data": [base64::engine::general_purpose::STANDARD.encode(&account.data), "base64"] })
            }).collect();
            let body = serde_json::json!({ "jsonrpc": "2.0", "id": 1,
                "result": { "context": { "slot": slot }, "value": values } }).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        }
    });
    let snapshot = snapshot::fetch_snapshot(&RpcClient::new(endpoint), &pools, &config).await.unwrap();
    assert_eq!((snapshot.min_slot, snapshot.max_slot), (100, 100));
    assert!(snapshot.pools.iter().all(Result::is_ok));
    server.join().unwrap();
}

#[test]
fn compare_local_quotes_with_saved_official_snapshot_references_when_available() {
    use base64::Engine;
    #[derive(Deserialize)]
    struct Reference {
        sdk: String,
        sdk_version: String,
        timestamp: i64,
        pool: crate::config::PoolConfig,
        quote_accounts: QuoteAccountsConfig,
        accounts: Vec<SavedAccount>,
        quotes: Vec<Expected>,
    }
    #[derive(Deserialize)]
    struct SavedAccount { address: String, owner: String, data_base64: String, slot: u64 }
    #[derive(Deserialize)]
    struct Expected { input_raw: u64, buy: bool, output_raw: u64, fee_raw: u64, consumed_raw: u64, complete: bool }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/local/arbitrage_quotes.json");
    if !path.exists() {
        eprintln!("SKIPPED official arbitrage quote comparison: {} is missing", path.display());
        return;
    }
    let references: Vec<Reference> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for dex in [DexKind::Raydium, DexKind::Orca, DexKind::MeteoraDlmm] {
        assert!(references.iter().any(|r| r.pool.dex == dex), "reference fixture must cover {dex}");
    }
    for reference in references {
        assert!(!reference.sdk.is_empty() && !reference.sdk_version.is_empty());
        assert!(reference.quotes.iter().any(|q| q.buy));
        assert!(reference.quotes.iter().any(|q| !q.buy));
        let mut config = cfg();
        config.accounts = vec![reference.quote_accounts];
        let accounts: std::collections::HashMap<_, _> = reference.accounts.into_iter().map(|a| {
            (a.address.clone(), AccountData { address: a.address, owner: a.owner, lamports: 0,
                data: base64::engine::general_purpose::STANDARD.decode(a.data_base64).unwrap(), slot: a.slot })
        }).collect();
        let pool = snapshot::decode(&reference.pool, &config, &accounts, reference.timestamp).unwrap();
        for expected in reference.quotes {
            let quote = pool.quote(expected.input_raw, expected.buy).unwrap();
            assert_eq!(quote.output_raw, expected.output_raw, "{} output", reference.sdk);
            assert_eq!(quote.fee_raw, expected.fee_raw, "{} fee", reference.sdk);
            assert_eq!(quote.consumed_raw, expected.consumed_raw, "{} consumed input", reference.sdk);
            assert_eq!(quote.complete, expected.complete, "{} full fill", reference.sdk);
        }
    }
}

use super::{config::ArbitrageConfig, local::{Bin, LocalPool, PoolMath, Tick, VariableFee}, math::*};
use crate::{config::PoolConfig, dex::{self, DexKind}, errors::AppError, rpc::{AccountData, RpcClient}};
use std::collections::HashMap;

const CLOCK: &str = "SysvarC1ock11111111111111111111111111111111";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// pool・mint・vault・配列・Clockをまとめて取得した裁定判定専用の状態。
pub struct Snapshot {
    pub pools: Vec<Result<LocalPool, AppError>>,
    pub min_slot: u64,
    pub max_slot: u64,
}

/// 全依存アカウントのslot差を検証する。最大slotだけでは古いvaultを見落とす。
pub fn slot_range(accounts: &[AccountData], allowed: u64) -> Result<(u64, u64), AppError> {
    let min = accounts.iter().map(|a| a.slot).min().ok_or_else(|| error("empty snapshot"))?;
    let max = accounts.iter().map(|a| a.slot).max().ok_or_else(|| error("empty snapshot"))?;
    if min == 0 || max - min > allowed {
        return Err(AppError::Pricing(format!("inconsistent snapshot slots: min={min}, max={max}, allowed={allowed}")));
    }
    Ok((min, max))
}

/// 整合しなければpool発見から全状態を1回だけ取り直す。送信直前再quoteとは別の処理。
pub async fn fetch_snapshot(
    rpc: &RpcClient, pools: &[PoolConfig], config: &ArbitrageConfig,
) -> Result<Snapshot, AppError> {
    match fetch_once(rpc, pools, config).await {
        Ok(snapshot) => Ok(snapshot),
        Err(_) => fetch_once(rpc, pools, config).await,
    }
}

async fn fetch_once(rpc: &RpcClient, pools: &[PoolConfig], config: &ArbitrageConfig) -> Result<Snapshot, AppError> {
    let enabled: Vec<_> = pools.iter().filter(|p| p.enabled).collect();
    let addresses: Vec<_> = enabled.iter().map(|p| p.account_address()).collect();
    let discovery = fetch_accounts(rpc, &addresses).await?;
    let mut requested = addresses;
    requested.push(CLOCK.into());
    // 初回のpool取得は依存先の発見にだけ使う。quoteには最後の一括取得のpoolを使う。
    for (pool, account) in enabled.iter().zip(&discovery) {
        let entry = config.accounts.iter().find(|c| c.pool_address == pool.account_address())
            .ok_or_else(|| error("missing arbitrage account configuration"))?;
        if account.owner != entry.program_id { return Err(error("pool program owner mismatch")); }
        requested.push(pool.base_mint.clone());
        requested.push(pool.quote_mint.clone());
        match pool.dex {
            DexKind::Raydium => {
                let meta = dex::raydium::decode_pool_meta(&account.data)?;
                requested.extend([meta.base_vault, meta.quote_vault]);
            }
            DexKind::Orca => {
                let meta = dex::orca::decode_pool_meta(&account.data)?;
                requested.extend([meta.token_vault_a, meta.token_vault_b]);
            }
            DexKind::MeteoraDlmm => {
                let meta = dex::meteora::decode_pool_meta(&account.data)?;
                requested.extend([meta.reserve_x, meta.reserve_y]);
            }
        }
        requested.extend(entry.arrays.iter().cloned());
    }
    requested.sort();
    requested.dedup();
    if requested.len() > 100 { return Err(error("snapshot exceeds getMultipleAccounts limit")); }
    let fetched = fetch_accounts(rpc, &requested).await?;
    let (min_slot, max_slot) = slot_range(&fetched, config.max_slot_difference)?;
    let accounts: HashMap<_, _> = fetched.into_iter().map(|a| (a.address.clone(), a)).collect();
    let timestamp = i64_at(&get(&accounts, CLOCK)?.data, 32)?;
    let decoded = enabled.into_iter().map(|pool| decode(pool, config, &accounts, timestamp)).collect();
    Ok(Snapshot { pools: decoded, min_slot, max_slot })
}

// 候補専用RPCの待機に上限を設け、価格監視ループが無期限に止まるのを防ぐ。
async fn fetch_accounts(rpc: &RpcClient, addresses: &[String]) -> Result<Vec<AccountData>, AppError> {
    tokio::time::timeout(std::time::Duration::from_secs(10), rpc.get_multiple_accounts(addresses))
        .await.map_err(|_| AppError::Rpc("arbitrage snapshot request timed out".into()))?
}

fn get<'a>(accounts: &'a HashMap<String, AccountData>, address: &str) -> Result<&'a AccountData, AppError> {
    accounts.get(address).ok_or_else(|| error("snapshot dependency changed or is missing"))
}

fn mint(account: &AccountData, decimals: u8) -> Result<(), AppError> {
    if account.owner != TOKEN || account.data.len() != 82
        || account.data[44] != decimals || account.data[45] != 1 {
        return Err(error("local quote requires initialized classic SPL SOL/USDC mints"));
    }
    Ok(())
}

fn vault(accounts: &HashMap<String, AccountData>, address: &str, expected_mint: &str) -> Result<u64, AppError> {
    let a = get(accounts, address)?;
    if a.owner != TOKEN || a.data.len() != 165 || a.data[108] != 1
        || dex::read_pubkey(&a.data, 0, "vault mint")? != expected_mint {
        return Err(error("invalid or frozen quote vault"));
    }
    u64_at(&a.data, 64)
}

pub(super) fn decode(pool: &PoolConfig, config: &ArbitrageConfig,
    accounts: &HashMap<String, AccountData>, timestamp: i64) -> Result<LocalPool, AppError> {
    dex::require_sol_usdc(&pool.pair, &pool.base_mint, &pool.quote_mint)?;
    mint(get(accounts, &pool.base_mint)?, 9)?;
    mint(get(accounts, &pool.quote_mint)?, 6)?;
    let entry = config.accounts.iter().find(|c| c.pool_address == pool.account_address())
        .ok_or_else(|| error("missing quote account configuration"))?;
    let account = get(accounts, &entry.pool_address)?;
    if account.owner != entry.program_id { return Err(error("pool owner changed")); }
    let arrays: Vec<_> = entry.arrays.iter().map(|a| get(accounts, a)).collect::<Result<_, _>>()?;
    if arrays.iter().any(|a| a.owner != entry.program_id) { return Err(error("array owner mismatch")); }
    let d = &account.data;
    let math = match pool.dex {
        DexKind::Raydium => {
            // 現行AMM v4のswap_base_in_v2と同じ、vaultから未回収PnLを除いた準備金。
            // CLMM/CPMMや稼働前poolはこのレイアウトで解釈しない。
            if d.len() != 752 || !matches!(u64_at(d, 0)?, 1 | 6) { return Err(error("unsupported Raydium layout or status")); }
            let m = dex::raydium::decode_pool_meta(d)?;
            if m.base_mint != pool.base_mint || m.quote_mint != pool.quote_mint
                || u64_at(d, 32)? != 9 || u64_at(d, 40)? != 6
                || m.swap_fee_denominator == 0 || m.swap_fee_numerator >= m.swap_fee_denominator {
                return Err(error("invalid Raydium mints, decimals or fee"));
            }
            let sol = vault(accounts, &m.base_vault, &pool.base_mint)?.checked_sub(u64_at(d, 192)?)
                .ok_or_else(|| error("Raydium SOL PnL exceeds reserve"))?;
            let usdc = vault(accounts, &m.quote_vault, &pool.quote_mint)?.checked_sub(u64_at(d, 200)?)
                .ok_or_else(|| error("Raydium USDC PnL exceeds reserve"))?;
            if sol == 0 || usdc == 0 { return Err(error("empty Raydium reserve")); }
            PoolMath::Raydium { sol, usdc, fee_n: m.swap_fee_numerator, fee_d: m.swap_fee_denominator }
        }
        DexKind::Orca => {
            if d.len() != 653 || bytes::<8>(d, 0)? != [63, 149, 209, 12, 225, 128, 99, 9] {
                return Err(error("unsupported Whirlpool layout"));
            }
            let m = dex::orca::decode_pool_meta(d)?;
            let spacing = i32::from(u16_at(d, 41)?);
            if spacing == 0 || u16_at(d, 43)? != spacing as u16 {
                return Err(error("adaptive-fee Whirlpool is not supported by this quote version"));
            }
            let sol_is_a = m.token_mint_a == pool.base_mint && m.token_mint_b == pool.quote_mint;
            if !sol_is_a && !(m.token_mint_a == pool.quote_mint && m.token_mint_b == pool.base_mint) {
                return Err(error("Whirlpool mints mismatch"));
            }
            vault(accounts, &m.token_vault_a, &m.token_mint_a)?;
            vault(accounts, &m.token_vault_b, &m.token_mint_b)?;
            let current_tick = i32_at(d, 81)?;
            if !(-443636..=443636).contains(&current_tick) || m.sqrt_price < tick_sqrt(current_tick)?
                || (current_tick < 443636 && m.sqrt_price > tick_sqrt(current_tick + 1)?) {
                return Err(error("Whirlpool tick and sqrt price disagree"));
            }
            let width = spacing * 88;
            let mut starts = Vec::new();
            let mut ticks = Vec::new();
            for array in arrays {
                let a = &array.data;
                if a.len() != 9988 || bytes::<8>(a, 0)? != [69, 97, 189, 190, 110, 7, 66, 187]
                    || dex::read_pubkey(a, 9956, "tick array pool")? != account.address {
                    return Err(error("unsupported or unrelated fixed TickArray"));
                }
                let start = i32_at(a, 8)?;
                if start.rem_euclid(width) != 0 || start < -443636 - width || start > 443636 {
                    return Err(error("invalid tick array start"));
                }
                starts.push(start);
                for i in 0..88 {
                    let offset = 12 + i * 113;
                    let initialized = a[offset];
                    if initialized > 1 { return Err(error("invalid initialized tick flag")); }
                    if initialized == 1 {
                        let index = start + i as i32 * spacing;
                        tick_sqrt(index)?;
                        ticks.push(Tick { index, liquidity_net: i128::from_le_bytes(bytes(a, offset + 1)?) });
                    }
                }
            }
            starts.sort();
            ticks.sort_by_key(|t| t.index);
            if starts.is_empty() || starts.windows(2).any(|w| w[1] - w[0] != width) {
                return Err(error("Whirlpool tick arrays must be contiguous without duplicates"));
            }
            let lower_tick = starts[0].max(-443636);
            let upper_tick = (starts[starts.len() - 1] + width).min(443636);
            if current_tick < lower_tick || current_tick >= upper_tick {
                return Err(error("current Whirlpool tick is outside configured arrays"));
            }
            PoolMath::Orca { sol_is_a, sqrt: m.sqrt_price, liquidity: u128_at(d, 49)?, current_tick,
                fee: u16_at(d, 45)?, lower_tick, upper_tick, ticks }
        }
        DexKind::MeteoraDlmm => {
            if d.len() != 904 || bytes::<8>(d, 0)? != [33, 11, 49, 98, 181, 101, 177, 13]
                || d[75] != 0 || d[82] != 0 || d[35] != 0 || d[36] != 0 {
                return Err(error("local DLMM quote requires an enabled permissionless pool with standard input fees"));
            }
            let m = dex::meteora::decode_pool_meta(d)?;
            if m.bin_step == 0 || m.base_fee_power_factor > 10
                || u16_at(d, 14)? > 10_000 || u16_at(d, 12)? < u16_at(d, 10)? {
                return Err(error("invalid DLMM fee parameters"));
            }
            let sol_is_x = m.token_x_mint == pool.base_mint && m.token_y_mint == pool.quote_mint;
            if !sol_is_x && !(m.token_x_mint == pool.quote_mint && m.token_y_mint == pool.base_mint) {
                return Err(error("DLMM mints mismatch"));
            }
            vault(accounts, &m.reserve_x, &m.token_x_mint)?;
            vault(accounts, &m.reserve_y, &m.token_y_mint)?;
            let mut starts = Vec::new();
            let mut bins = Vec::new();
            for array in arrays {
                let a = &array.data;
                if a.len() != 10136 || bytes::<8>(a, 0)? != [92, 142, 92, 220, 5, 148, 70, 181]
                    || dex::read_pubkey(a, 24, "bin array pool")? != account.address {
                    return Err(error("unsupported or unrelated BinArray"));
                }
                let start = i64_at(a, 8)?.checked_mul(70).and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(|| error("bin array index overflow"))?;
                if !(-500000..=500000).contains(&start) { return Err(error("bin array outside supported ID range")); }
                starts.push(start);
                for i in 0..70 {
                    let o = 56 + i * 144;
                    if u64_at(a, o + 112)? != 0 || u64_at(a, o + 128)? != 0 {
                        return Err(error("DLMM limit-order liquidity requires a newer quote version"));
                    }
                    let bin = Bin { id: start + i as i32, x: u64_at(a, o)?, y: u64_at(a, o + 8)?, price: u128_at(a, o + 16)? };
                    if bin.price == 0 && (bin.x != 0 || bin.y != 0) { return Err(error("funded bin has zero price")); }
                    bins.push(bin);
                }
            }
            starts.sort();
            bins.sort_by_key(|b| b.id);
            if starts.is_empty() || starts.windows(2).any(|w| w[1] - w[0] != 70)
                || m.active_id < starts[0] || m.active_id >= starts[starts.len() - 1] + 70 {
                return Err(error("DLMM arrays must cover active bin and be contiguous without duplicates"));
            }
            let elapsed = timestamp.checked_sub(i64_at(d, 56)?).filter(|e| *e >= 0)
                .ok_or_else(|| error("DLMM fee timestamp is newer than snapshot Clock"))?;
            let mut reference = u32_at(d, 44)?;
            let mut index_reference = i32_at(d, 48)?;
            if elapsed >= i64::from(u16_at(d, 10)?) {
                index_reference = m.active_id;
                reference = if elapsed < i64::from(u16_at(d, 12)?) {
                    (u64::from(m.volatility_accumulator) * u64::from(u16_at(d, 14)?) / 10_000) as u32
                } else { 0 };
            }
            let fee = VariableFee { base: u128::from(m.base_fee_factor) * u128::from(m.bin_step)
                    * 10 * 10u128.pow(u32::from(m.base_fee_power_factor)),
                control: m.variable_fee_control, bin_step: m.bin_step, reference,
                index_reference, max_accumulator: u32_at(d, 20)? };
            PoolMath::Meteora { sol_is_x, active_id: m.active_id, bins, fee }
        }
    };
    Ok(LocalPool { dex: pool.dex, address: account.address.clone(), math })
}

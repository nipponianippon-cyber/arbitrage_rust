mod config;
mod local;
mod math;
pub mod snapshot;

pub use config::{ArbitrageConfig, FixedCosts, QuoteAccountsConfig, usdc_raw};
pub use local::{LocalPool, LocalQuote};

use crate::{config::AppConfig, errors::AppError, notifier::DiscordNotifier, rpc::RpcClient, storage::Storage};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 探索した入力ごとにquoteの成功・失敗と整数利益を残す。
#[derive(Debug, Clone, Serialize)]
pub struct QuoteTrial {
    pub input_raw: u64,
    pub buy: Option<LocalQuote>,
    pub sell: Option<LocalQuote>,
    pub profit_raw: Option<i128>,
    pub error: Option<String>,
}

/// 1サイクル・1方向の評価。再quote・transaction状態はマイルストーン11以降で追加する。
#[derive(Debug, Clone, Serialize)]
pub struct CandidateEvaluation {
    pub observed_at: String,
    pub route_key: String,
    pub buy_dex: String,
    pub sell_dex: String,
    pub buy_pool: String,
    pub sell_pool: String,
    pub state: String,
    pub input_raw: Option<u64>,
    pub output_raw: Option<u64>,
    pub expected_profit_raw: Option<i128>,
    pub min_profit_raw: u64,
    pub fixed_costs: FixedCosts,
    pub fixed_cost_raw: u64,
    pub min_slot: Option<u64>,
    pub max_slot: Option<u64>,
    pub trials: Vec<QuoteTrial>,
    pub error: Option<String>,
}

/// 最後に通知できた候補状態。DBへ保存して再起動後も重複を抑える。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationState {
    pub state: String,
    pub profit_raw: Option<i128>,
}

impl CandidateEvaluation {
    /// 小さな毎回の利益変動は通知せず、最後の通知値からの累積変化を見る。
    pub fn should_notify(&self, previous: Option<&NotificationState>, change: u64) -> bool {
        match previous {
            None => self.state == "detected",
            Some(p) if p.state != self.state => self.state == "detected" || p.state == "detected",
            Some(p) if self.state == "detected" => match (self.expected_profit_raw, p.profit_raw) {
                (Some(now), Some(old)) => now.abs_diff(old) >= u128::from(change),
                _ => true,
            },
            _ => false,
        }
    }
}

fn trial(buy: &LocalPool, sell: &LocalPool, input: u64, cfg: &ArbitrageConfig, cost: u64) -> QuoteTrial {
    let mut result = QuoteTrial { input_raw: input, buy: None, sell: None, profit_raw: None, error: None };
    let run = (|| -> Result<(), AppError> {
        let first = buy.quote(input, true)?;
        result.buy = Some(first.clone());
        if !buy.within_impact(&first, true, cfg.max_price_impact_bps)? {
            return Err(math::error("buy leg partial fill, zero output or price impact limit"));
        }
        let second = sell.quote(first.output_raw, false)?;
        result.sell = Some(second.clone());
        if !sell.within_impact(&second, false, cfg.max_price_impact_bps)? {
            return Err(math::error("sell leg partial fill, zero output or price impact limit"));
        }
        result.profit_raw = Some(i128::from(second.output_raw) - i128::from(input) - i128::from(cost));
        Ok(())
    })();
    if let Err(e) = run { result.error = Some(e.to_string()); }
    result
}

// coarse探索の後、良い区間を複数残して再探索する。単峰性は仮定しない。
fn search_route(buy: &LocalPool, sell: &LocalPool, cfg: &ArbitrageConfig, cost: u64) -> Result<Vec<QuoteTrial>, AppError> {
    let min = usdc_raw(cfg.min_input_usdc)?;
    let max = usdc_raw(cfg.max_input_usdc)?;
    let mut samples = BTreeMap::new();
    for i in 0..cfg.coarse_points {
        let amount = min + (((max - min) as u128 * i as u128) / (cfg.coarse_points - 1) as u128) as u64;
        samples.entry(amount).or_insert_with(|| trial(buy, sell, amount, cfg, cost));
    }
    let mut boundary_queue = BTreeSet::new();
    let mut second_boundaries = BTreeSet::new();
    for sample in samples.values() {
        if let Some(q) = &sample.buy { boundary_queue.extend(q.boundary_inputs.iter().copied()); }
        if let Some(q) = &sample.sell { second_boundaries.extend(q.boundary_inputs.iter().copied()); }
    }
    // tick/bin端点の直前・直後は丸めで利益が変わるのでraw 1単位まで確認する。
    let boundaries: Vec<_> = boundary_queue.into_iter().flat_map(|n| [n.saturating_sub(1), n, n.saturating_add(1)]).collect();
    let boundary_budget = samples.len() + (cfg.max_route_quotes - samples.len()) / 3;
    for amount in boundaries {
        if samples.len() >= boundary_budget { break; }
        if (min..=max).contains(&amount) {
            samples.entry(amount).or_insert_with(|| trial(buy, sell, amount, cfg, cost));
        }
    }
    // 第2レッグの境界はSOL単位なので、第1レッグの単調な出力を使ってUSDC入力へ逆引きする。
    let inverse_budget = samples.len() + (cfg.max_route_quotes - samples.len()) / 3;
    for target in second_boundaries {
        while samples.len() < inverse_budget {
            let entries: Vec<_> = samples.iter().collect();
            let bracket = entries.windows(2).find_map(|w| {
                let (a, qa) = w[0]; let (b, qb) = w[1];
                let left = qa.buy.as_ref()?; let right = qb.buy.as_ref()?;
                if left.complete && right.complete && left.output_raw < target && right.output_raw >= target && b - a > 1 {
                    Some(*a + (b - a) / 2)
                } else { None }
            });
            let Some(amount) = bracket else { break; };
            samples.insert(amount, trial(buy, sell, amount, cfg, cost));
        }
    }
    let mut iteration = 0;
    while samples.len() < cfg.max_route_quotes {
        let entries: Vec<_> = samples.iter().collect();
        let mut intervals: Vec<_> = entries.windows(2).filter_map(|w| {
            let (a, qa) = w[0]; let (b, qb) = w[1];
            if b - a <= cfg.input_step_raw { return None; }
            let score = qa.profit_raw.into_iter().chain(qb.profit_raw).max().unwrap_or(i128::MIN);
            Some((*a, *b, score))
        }).collect();
        if intervals.is_empty() { break; }
        // 4回に1回は広い未探索区間を調べ、局所的なピークへの偏りを抑える。
        if iteration % 4 == 3 { intervals.sort_by_key(|(a, b, _)| std::cmp::Reverse(b - a)); }
        else { intervals.sort_by_key(|(a, b, score)| (std::cmp::Reverse(*score), std::cmp::Reverse(b - a))); }
        let (a, b, _) = intervals[0];
        let amount = a + (b - a) / 2;
        samples.insert(amount, trial(buy, sell, amount, cfg, cost));
        iteration += 1;
    }
    Ok(samples.into_values().collect())
}

fn evaluate(mut evaluation: CandidateEvaluation, buy: &LocalPool, sell: &LocalPool, cfg: &ArbitrageConfig) -> Result<CandidateEvaluation, AppError> {
    let (buy_n, buy_d) = buy.rate(true)?;
    let (sell_n, sell_d) = sell.rate(false)?;
    if buy_n * sell_n <= buy_d * sell_d {
        evaluation.state = "no_spread".into();
        return Ok(evaluation);
    }
    evaluation.trials = search_route(buy, sell, cfg, evaluation.fixed_cost_raw)?;
    let best = evaluation.trials.iter().filter(|t| t.profit_raw.is_some()).max_by(|a, b| {
        a.profit_raw.cmp(&b.profit_raw).then_with(|| b.input_raw.cmp(&a.input_raw))
    });
    if let Some(best) = best {
        evaluation.input_raw = Some(best.input_raw);
        evaluation.output_raw = best.sell.as_ref().map(|q| q.output_raw);
        evaluation.expected_profit_raw = best.profit_raw;
        evaluation.state = if best.profit_raw.unwrap_or(i128::MIN) > i128::from(evaluation.min_profit_raw) {
            "detected"
        } else { "unprofitable" }.into();
    } else {
        evaluation.state = "quote_failed".into();
        evaluation.error = Some("no fully executable quote within input/impact limits".into());
    }
    Ok(evaluation)
}

/// 全3ペアの両方向を保存する。候補判定失敗は価格監視の停止理由にしない。
pub async fn run_cycle(app: &AppConfig, rpc: &RpcClient, storage: &Storage, notifier: &DiscordNotifier) -> Result<(), AppError> {
    let Some(cfg) = app.arbitrage.as_ref().filter(|c| c.enabled) else { return Ok(()); };
    let snapshot = snapshot::fetch_snapshot(rpc, &app.pools, cfg).await;
    let enabled: Vec<_> = app.pools.iter().filter(|p| p.enabled).collect();
    let cost = cfg.costs.total_raw()?;
    let min_profit_raw = usdc_raw(cfg.min_profit_usdc)?;
    let change = usdc_raw(cfg.notification_profit_change_usdc)?;
    let observed_at = Utc::now().to_rfc3339();
    for (i, buy) in enabled.iter().enumerate() {
        for (j, sell) in enabled.iter().enumerate() {
            if i == j { continue; }
            let mut record = CandidateEvaluation {
                observed_at: observed_at.clone(), route_key: format!("SOL/USDC:{}>{}", buy.account_address(), sell.account_address()),
                buy_dex: buy.dex.to_string(), sell_dex: sell.dex.to_string(),
                buy_pool: buy.account_address(), sell_pool: sell.account_address(), state: "quote_failed".into(),
                input_raw: None, output_raw: None, expected_profit_raw: None, min_profit_raw,
                fixed_costs: cfg.costs.clone(), fixed_cost_raw: cost, min_slot: None, max_slot: None,
                trials: vec![], error: None,
            };
            match &snapshot {
                Err(e) => { record.error = Some(e.to_string()); }
                Ok(s) => {
                    record.min_slot = Some(s.min_slot); record.max_slot = Some(s.max_slot);
                    match (&s.pools[i], &s.pools[j]) {
                        (Ok(b), Ok(s)) => match evaluate(record.clone(), b, s, cfg) {
                            Ok(evaluated) => record = evaluated,
                            Err(e) => record.error = Some(e.to_string()),
                        },
                        (Err(e), _) | (_, Err(e)) => record.error = Some(e.to_string()),
                    }
                }
            }
            storage.insert_candidate_evaluation(&record)?;
            if app.notification.discord_enabled {
                let previous = storage.candidate_notification_state(&record.route_key)?;
                if record.should_notify(previous.as_ref(), change) {
                    match notifier.send_candidate(&record).await {
                        Ok(()) => storage.save_candidate_notification_state(&record)?,
                        Err(e) => storage.insert_monitor_error(&e.to_monitor_record())?,
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

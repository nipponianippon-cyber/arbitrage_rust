use crate::dex::DexPrice;
use crate::dex::meteora::{MeteoraDlmmQuote, MeteoraDlmmState};
use crate::errors::{AppError, MonitorErrorRecord};
use crate::pricing::PriceSpread;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(Self {
            conn: Connection::open(path)?,
        })
    }

    pub fn init_schema(&self) -> Result<(), AppError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS price_observations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                observed_at TEXT NOT NULL,
                dex TEXT NOT NULL,
                pair TEXT NOT NULL,
                pool_address TEXT NOT NULL,
                lb_pair_address TEXT,
                price TEXT,
                fee_adjusted_price TEXT,
                slippage_adjusted_price TEXT,
                liquidity TEXT,
                slot INTEGER,
                rpc_success INTEGER NOT NULL,
                error_kind TEXT,
                error_message TEXT
            );

            CREATE TABLE IF NOT EXISTS price_spreads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                calculated_at TEXT NOT NULL,
                pair TEXT NOT NULL,
                raydium_price TEXT NOT NULL,
                orca_price TEXT NOT NULL,
                absolute_spread TEXT NOT NULL,
                spread_bps TEXT NOT NULL,
                higher_dex TEXT NOT NULL,
                lower_dex TEXT NOT NULL,
                comparison_direction TEXT,
                fee_adjusted_reference_spread TEXT
            );

            CREATE TABLE IF NOT EXISTS monitor_errors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                occurred_at TEXT NOT NULL,
                component TEXT NOT NULL,
                severity TEXT NOT NULL,
                message TEXT NOT NULL,
                source TEXT,
                dex TEXT,
                pool_address TEXT,
                retry_planned INTEGER NOT NULL DEFAULT 1,
                consecutive_count INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS price_spread_pairs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                calculated_at TEXT NOT NULL,
                pair TEXT NOT NULL,
                dex_a TEXT NOT NULL,
                dex_b TEXT NOT NULL,
                dex_a_price TEXT NOT NULL,
                dex_b_price TEXT NOT NULL,
                absolute_spread TEXT NOT NULL,
                spread_bps TEXT NOT NULL,
                higher_dex TEXT NOT NULL,
                lower_dex TEXT NOT NULL,
                comparison_direction TEXT,
                fee_adjusted_reference_spread TEXT
            );

            CREATE TABLE IF NOT EXISTS meteora_dlmm_states (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                observed_at TEXT NOT NULL,
                lb_pair_address TEXT NOT NULL,
                active_id INTEGER NOT NULL,
                bin_step INTEGER NOT NULL,
                token_x_mint TEXT NOT NULL,
                token_y_mint TEXT NOT NULL,
                base_fee_bps TEXT,
                variable_fee_bps TEXT,
                total_fee_bps TEXT,
                status TEXT,
                liquidity TEXT,
                slot INTEGER
            );

            CREATE TABLE IF NOT EXISTS meteora_dlmm_quotes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                observed_at TEXT NOT NULL,
                lb_pair_address TEXT NOT NULL,
                direction TEXT NOT NULL,
                input_mint TEXT NOT NULL,
                output_mint TEXT NOT NULL,
                requested_input_amount TEXT NOT NULL,
                requested_input_amount_raw TEXT NOT NULL,
                consumed_input_amount TEXT,
                consumed_input_amount_raw TEXT,
                output_amount TEXT,
                output_amount_raw TEXT,
                fee_amount TEXT,
                fee_amount_raw TEXT,
                protocol_fee_amount TEXT,
                protocol_fee_amount_raw TEXT,
                price_impact_bps TEXT,
                effective_price TEXT,
                end_price TEXT,
                bin_array_count INTEGER NOT NULL,
                bin_array_addresses TEXT NOT NULL,
                partial_fill INTEGER NOT NULL,
                success INTEGER NOT NULL,
                error_message TEXT,
                slot INTEGER
            );
            ",
        )?;
        self.add_column_if_missing("price_observations", "lb_pair_address", "TEXT")?;
        self.add_column_if_missing("price_observations", "slippage_adjusted_price", "TEXT")?;
        self.add_column_if_missing("price_spreads", "comparison_direction", "TEXT")?;
        self.add_column_if_missing("price_spreads", "fee_adjusted_reference_spread", "TEXT")?;
        self.add_column_if_missing("monitor_errors", "dex", "TEXT")?;
        self.add_column_if_missing("monitor_errors", "pool_address", "TEXT")?;
        self.add_column_if_missing(
            "monitor_errors",
            "retry_planned",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        self.add_column_if_missing(
            "monitor_errors",
            "consecutive_count",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        Ok(())
    }

    pub fn insert_price_observation(&self, price: &DexPrice) -> Result<(), AppError> {
        self.conn.execute(
            "
            INSERT INTO price_observations (
                observed_at, dex, pair, pool_address, lb_pair_address, price,
                fee_adjusted_price, slippage_adjusted_price, liquidity, slot,
                rpc_success, error_kind, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, NULL, NULL)
            ",
            params![
                price.observed_at.to_rfc3339(),
                price.dex.as_str(),
                price.pair.as_str(),
                price.pool_address.as_str(),
                lb_pair_address_for(price.dex, price.pool_address.as_str()),
                price.price.to_string(),
                price.fee_adjusted_price.map(|value| value.to_string()),
                price.slippage_adjusted_price.map(|value| value.to_string()),
                price.liquidity.map(|value| value.to_string()),
                price.slot,
            ],
        )?;
        Ok(())
    }

    pub fn insert_failed_price_observation(
        &self,
        dex: &str,
        pair: &str,
        pool_address: &str,
        error: &AppError,
    ) -> Result<(), AppError> {
        self.conn.execute(
            "
            INSERT INTO price_observations (
                observed_at, dex, pair, pool_address, lb_pair_address, price,
                fee_adjusted_price, slippage_adjusted_price, liquidity, slot,
                rpc_success, error_kind, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL, NULL, NULL, 0, ?6, ?7)
            ",
            params![
                chrono::Utc::now().to_rfc3339(),
                dex,
                pair,
                pool_address,
                lb_pair_address_for_str(dex, pool_address),
                error.component(),
                error.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn insert_price_spread(&self, spread: &PriceSpread) -> Result<(), AppError> {
        self.insert_price_spread_pair(spread)?;
        if spread.dex_a.dex != crate::dex::DexKind::Raydium
            || spread.dex_b.dex != crate::dex::DexKind::Orca
        {
            return Ok(());
        }
        let raydium_price = price_for_dex(spread, "Raydium")?;
        let orca_price = price_for_dex(spread, "Orca")?;
        self.conn.execute(
            "
            INSERT INTO price_spreads (
                calculated_at, pair, raydium_price, orca_price, absolute_spread,
                spread_bps, higher_dex, lower_dex, comparison_direction,
                fee_adjusted_reference_spread
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                spread.calculated_at.to_rfc3339(),
                spread.pair.as_str(),
                raydium_price.to_string(),
                orca_price.to_string(),
                spread.absolute_spread.to_string(),
                spread.spread_bps.to_string(),
                spread.higher_dex.as_str(),
                spread.lower_dex.as_str(),
                spread.comparison_direction.as_str(),
                spread
                    .fee_adjusted_reference_spread
                    .map(|value| value.to_string()),
            ],
        )?;
        Ok(())
    }

    pub fn insert_price_spread_pair(&self, spread: &PriceSpread) -> Result<(), AppError> {
        self.conn.execute(
            "
            INSERT INTO price_spread_pairs (
                calculated_at, pair, dex_a, dex_b, dex_a_price, dex_b_price,
                absolute_spread, spread_bps, higher_dex, lower_dex,
                comparison_direction, fee_adjusted_reference_spread
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ",
            params![
                spread.calculated_at.to_rfc3339(),
                spread.pair.as_str(),
                spread.dex_a.dex.as_str(),
                spread.dex_b.dex.as_str(),
                spread.dex_a.price.to_string(),
                spread.dex_b.price.to_string(),
                spread.absolute_spread.to_string(),
                spread.spread_bps.to_string(),
                spread.higher_dex.as_str(),
                spread.lower_dex.as_str(),
                spread.comparison_direction.as_str(),
                spread
                    .fee_adjusted_reference_spread
                    .map(|value| value.to_string()),
            ],
        )?;
        Ok(())
    }

    pub fn insert_meteora_dlmm_state(&self, state: &MeteoraDlmmState) -> Result<(), AppError> {
        self.conn.execute(
            "
            INSERT INTO meteora_dlmm_states (
                observed_at, lb_pair_address, active_id, bin_step, token_x_mint,
                token_y_mint, base_fee_bps, variable_fee_bps, total_fee_bps,
                status, liquidity, slot
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ",
            params![
                state.observed_at.to_rfc3339(),
                state.lb_pair_address.as_str(),
                state.active_id,
                state.bin_step,
                state.token_x_mint.as_str(),
                state.token_y_mint.as_str(),
                state.base_fee_bps.map(|value| value.to_string()),
                state.variable_fee_bps.map(|value| value.to_string()),
                state.total_fee_bps.map(|value| value.to_string()),
                state.status.as_deref(),
                state.liquidity.map(|value| value.to_string()),
                state.slot,
            ],
        )?;
        Ok(())
    }

    pub fn insert_meteora_dlmm_quote(&self, quote: &MeteoraDlmmQuote) -> Result<(), AppError> {
        let bin_array_addresses = serde_json::to_string(&quote.bin_array_addresses)?;
        self.conn.execute(
            "
            INSERT INTO meteora_dlmm_quotes (
                observed_at, lb_pair_address, direction, input_mint, output_mint,
                requested_input_amount, requested_input_amount_raw,
                consumed_input_amount, consumed_input_amount_raw,
                output_amount, output_amount_raw, fee_amount, fee_amount_raw,
                protocol_fee_amount, protocol_fee_amount_raw, price_impact_bps,
                effective_price, end_price, bin_array_count, bin_array_addresses,
                partial_fill, success, error_message, slot
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24
            )
            ",
            params![
                quote.observed_at.to_rfc3339(),
                quote.lb_pair_address.as_str(),
                quote.direction.as_str(),
                quote.input_mint.as_str(),
                quote.output_mint.as_str(),
                quote.requested_input_amount.to_string(),
                quote.requested_input_amount_raw.to_string(),
                quote.consumed_input_amount.map(|value| value.to_string()),
                quote.consumed_input_amount_raw.map(|value| value.to_string()),
                quote.output_amount.map(|value| value.to_string()),
                quote.output_amount_raw.map(|value| value.to_string()),
                quote.fee_amount.map(|value| value.to_string()),
                quote.fee_amount_raw.map(|value| value.to_string()),
                quote.protocol_fee_amount.map(|value| value.to_string()),
                quote
                    .protocol_fee_amount_raw
                    .map(|value| value.to_string()),
                quote.price_impact_bps.map(|value| value.to_string()),
                quote.effective_price.map(|value| value.to_string()),
                quote.end_price.map(|value| value.to_string()),
                quote.bin_array_count as i64,
                bin_array_addresses,
                quote.partial_fill as i32,
                quote.success as i32,
                quote.error_message.as_deref(),
                quote.slot,
            ],
        )?;
        Ok(())
    }

    pub fn insert_monitor_error(&self, error: &MonitorErrorRecord) -> Result<(), AppError> {
        self.conn.execute(
            "
            INSERT INTO monitor_errors (
                occurred_at, component, severity, message, source, dex,
                pool_address, retry_planned, consecutive_count
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                error.occurred_at.to_rfc3339(),
                error.component.as_str(),
                error.severity.as_str(),
                error.message.as_str(),
                error.source.as_deref(),
                error.dex.map(|dex| dex.as_str()),
                error.pool_address.as_deref(),
                error.retry_planned,
                error.consecutive_count,
            ],
        )?;
        Ok(())
    }

    fn add_column_if_missing(
        &self,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<(), AppError> {
        let exists: Option<String> = self
            .conn
            .query_row(&format!("PRAGMA table_info({table})"), [], |_| {
                Ok(String::new())
            })
            .optional()?;
        if exists.is_none() {
            return Ok(());
        }

        let mut statement = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let existing: String = row.get(1)?;
            if existing == column {
                return Ok(());
            }
        }

        self.conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
        Ok(())
    }
}

fn price_for_dex(spread: &PriceSpread, dex: &str) -> Result<rust_decimal::Decimal, AppError> {
    if spread.dex_a.dex.as_str() == dex {
        Ok(spread.dex_a.price)
    } else if spread.dex_b.dex.as_str() == dex {
        Ok(spread.dex_b.price)
    } else {
        Err(AppError::Pricing(format!("spread is missing {dex} price")))
    }
}

fn lb_pair_address_for(dex: crate::dex::DexKind, pool_address: &str) -> Option<&str> {
    if dex == crate::dex::DexKind::MeteoraDlmm {
        Some(pool_address)
    } else {
        None
    }
}

fn lb_pair_address_for_str<'a>(dex: &str, pool_address: &'a str) -> Option<&'a str> {
    if dex == crate::dex::DexKind::MeteoraDlmm.as_str() {
        Some(pool_address)
    } else {
        None
    }
}

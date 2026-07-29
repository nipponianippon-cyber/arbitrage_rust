use crate::dex::DexPrice;
use crate::errors::{AppError, MonitorErrorRecord};
use crate::pricing::PriceSpread;
use rusqlite::{params, Connection, OptionalExtension};
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
                price TEXT,
                fee_adjusted_price TEXT,
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
            ",
        )?;
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
                observed_at, dex, pair, pool_address, price, fee_adjusted_price,
                liquidity, slot, rpc_success, error_kind, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, NULL, NULL)
            ",
            params![
                price.observed_at.to_rfc3339(),
                price.dex.as_str(),
                price.pair.as_str(),
                price.pool_address.as_str(),
                price.price.to_string(),
                price.fee_adjusted_price.map(|value| value.to_string()),
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
                observed_at, dex, pair, pool_address, price, fee_adjusted_price,
                liquidity, slot, rpc_success, error_kind, error_message
            ) VALUES (?1, ?2, ?3, ?4, NULL, NULL, NULL, NULL, 0, ?5, ?6)
            ",
            params![
                chrono::Utc::now().to_rfc3339(),
                dex,
                pair,
                pool_address,
                error.component(),
                error.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn insert_price_spread(&self, spread: &PriceSpread) -> Result<(), AppError> {
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
            .query_row(
                &format!("PRAGMA table_info({table})"),
                [],
                |_| Ok(String::new()),
            )
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

        self.conn
            .execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"), [])?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::path::Path;
    use std::str::FromStr;

    const SDK_FIXTURE_PATH: &str = "tests/fixtures/meteora_dlmm_active_bin_sdk.generated.json";

    #[derive(Debug, Deserialize)]
    struct SdkActiveBinFixtureFile {
        base_mint: String,
        quote_mint: String,
        fixtures: Vec<SdkActiveBinFixture>,
    }

    #[derive(Debug, Deserialize)]
    struct SdkActiveBinFixture {
        lb_pair_address: String,
        active_id: i32,
        bin_step: u16,
        token_x_mint: String,
        token_y_mint: String,
        token_x_decimals: u8,
        token_y_decimals: u8,
        normalized_usdc_per_sol: String,
        #[serde(default)]
        quotes: Vec<SdkFixtureQuote>,
    }

    #[derive(Debug, Deserialize)]
    struct SdkFixtureQuote {
        direction: String,
        requested_input_amount_raw: String,
        consumed_input_amount_raw: Option<String>,
        output_amount_raw: Option<String>,
        fee_amount_raw: Option<String>,
        protocol_fee_amount_raw: Option<String>,
        price_impact_bps: Option<String>,
        effective_price: Option<String>,
        bin_array_addresses: Vec<String>,
        success: bool,
    }

    fn account(data: Vec<u8>) -> AccountData {
        AccountData {
            address: "mint-address".to_string(),
            owner: "owner".to_string(),
            lamports: 0,
            data,
            slot: 1,
        }
    }

    #[test]
    fn mint_decimals_reads_initialized_mint_account() {
        let mut data = vec![0; MINT_ACCOUNT_MIN_LEN];
        data[MINT_DECIMALS_OFFSET] = 6;
        data[MINT_IS_INITIALIZED_OFFSET] = 1;

        assert_eq!(mint_decimals(&account(data), "mint").unwrap(), 6);
    }

    #[test]
    fn mint_decimals_rejects_uninitialized_mint_account() {
        let data = vec![0; MINT_ACCOUNT_MIN_LEN];

        assert!(mint_decimals(&account(data), "mint").is_err());
    }

    #[test]
    fn variable_fee_returns_zero_when_control_is_zero() {
        assert_eq!(variable_fee(0, 10, 25).unwrap(), Decimal::ZERO);
    }

    #[test]
    fn variable_fee_uses_squared_volatility_step_with_ceil() {
        assert_eq!(variable_fee(1, 10, 25).unwrap(), Decimal::ONE);
    }

    #[test]
    fn active_bin_price_matches_meteora_sdk_fixture_when_present() {
        let path = Path::new(SDK_FIXTURE_PATH);
        if !path.exists() {
            eprintln!(
                "skipping Meteora-DLMM SDK active bin fixture test because {SDK_FIXTURE_PATH} is absent"
            );
            return;
        }

        let body = std::fs::read_to_string(path).unwrap();
        let fixture: SdkActiveBinFixtureFile = serde_json::from_str(&body).unwrap();
        assert!(
            !fixture.fixtures.is_empty(),
            "Meteora-DLMM SDK active bin fixture contains no pool cases"
        );

        for case in &fixture.fixtures {
            let actual = normalized_active_bin_price(&fixture, case);
            let expected = Decimal::from_str(&case.normalized_usdc_per_sol).unwrap();
            assert!(
                expected > Decimal::ZERO,
                "Meteora-DLMM SDK fixture expected price must be positive for {}",
                case.lb_pair_address
            );
            let diff_bps = bps_diff(actual, expected);

            assert!(
                diff_bps <= sdk_active_bin_tolerance_bps(),
                "Meteora-DLMM active bin price differs from SDK fixture for {}: actual={}, expected={}, diff_bps={}",
                case.lb_pair_address,
                actual,
                expected,
                diff_bps
            );
        }
    }

    #[test]
    fn quote_fixture_contains_both_directions_when_present() {
        let path = Path::new(SDK_FIXTURE_PATH);
        if !path.exists() {
            eprintln!(
                "skipping Meteora-DLMM SDK quote fixture test because {SDK_FIXTURE_PATH} is absent"
            );
            return;
        }

        let body = std::fs::read_to_string(path).unwrap();
        let fixture: SdkActiveBinFixtureFile = serde_json::from_str(&body).unwrap();
        for case in &fixture.fixtures {
            if case.quotes.is_empty() {
                eprintln!(
                    "skipping Meteora-DLMM SDK quote fixture assertions for {} because quotes are absent",
                    case.lb_pair_address
                );
                continue;
            }
            assert!(
                case.quotes
                    .iter()
                    .any(|quote| quote.direction == "USDC -> SOL"),
                "Meteora-DLMM SDK quote fixture is missing USDC -> SOL for {}",
                case.lb_pair_address
            );
            assert!(
                case.quotes
                    .iter()
                    .any(|quote| quote.direction == "SOL -> USDC"),
                "Meteora-DLMM SDK quote fixture is missing SOL -> USDC for {}",
                case.lb_pair_address
            );
            for quote in case.quotes.iter().filter(|quote| quote.success) {
                let requested_input = u64_from_str_or_zero(&quote.requested_input_amount_raw);
                let consumed_input =
                    u64_from_optional_str(quote.consumed_input_amount_raw.as_deref()).unwrap_or(0);
                let output = u64_from_optional_str(quote.output_amount_raw.as_deref()).unwrap_or(0);
                assert!(
                    requested_input > 0 && consumed_input > 0 && output > 0,
                    "Meteora-DLMM SDK quote fixture has non-positive raw amounts for {} {}",
                    case.lb_pair_address,
                    quote.direction
                );
                assert!(
                    !quote.bin_array_addresses.is_empty(),
                    "Meteora-DLMM SDK quote fixture has no BinArray addresses for {} {}",
                    case.lb_pair_address,
                    quote.direction
                );
                assert!(
                    quote.effective_price
                        .as_deref()
                        .and_then(|value| Decimal::from_str(value).ok())
                        .is_some_and(|value| value > Decimal::ZERO),
                    "Meteora-DLMM SDK quote fixture has invalid effective price for {} {}",
                    case.lb_pair_address,
                    quote.direction
                );
                assert!(quote.fee_amount_raw.is_some());
                assert!(quote.protocol_fee_amount_raw.is_some());
                assert!(quote.price_impact_bps.is_some());
            }
        }
    }

    fn normalized_active_bin_price(
        fixture: &SdkActiveBinFixtureFile,
        case: &SdkActiveBinFixture,
    ) -> Decimal {
        // SDK fixture内の同一スナップショット入力から、監視実装と同じactive bin価格を再計算する。
        let raw_price = active_bin_price(
            case.active_id,
            case.bin_step,
            case.token_x_decimals,
            case.token_y_decimals,
        )
        .unwrap();

        if case.token_x_mint.as_str() == fixture.base_mint.as_str()
            && case.token_y_mint.as_str() == fixture.quote_mint.as_str()
        {
            raw_price
        } else if case.token_x_mint.as_str() == fixture.quote_mint.as_str()
            && case.token_y_mint.as_str() == fixture.base_mint.as_str()
        {
            Decimal::ONE / raw_price
        } else {
            panic!(
                "Meteora-DLMM SDK fixture token mints do not match configured base/quote for {}",
                case.lb_pair_address
            );
        }
    }

    fn bps_diff(actual: Decimal, expected: Decimal) -> Decimal {
        let diff = if actual >= expected {
            actual - expected
        } else {
            expected - actual
        };
        diff / expected * Decimal::from(10_000u64)
    }

    fn sdk_active_bin_tolerance_bps() -> Decimal {
        Decimal::new(1, 2)
    }
}

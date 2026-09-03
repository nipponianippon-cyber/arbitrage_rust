# Meteora-DLMM SDK Fixture

This helper generates the local SDK reference fixture used by the Rust offline tests for the Meteora-DLMM active bin price formula and both quote directions.

The generated fixture is intentionally not committed. It depends on the live RPC state at generation time and on the installed `@meteora-ag/dlmm` version.

## Usage

```powershell
cd tools/meteora-dlmm-sdk-fixture
npm install
$env:HELIUS_RPC_URL = "https://mainnet.helius-rpc.com/?api-key=..."
npm run generate -- --pool <LB_PAIR_ADDRESS_1> --pool <LB_PAIR_ADDRESS_2>
```

Optional quote controls:

```powershell
npm run generate -- --pool <LB_PAIR_ADDRESS> --trade-size-usdc 100 --bin-array-count 4 --slippage-bps 50
```

By default, the output is written to:

```text
../../tests/fixtures/meteora_dlmm_active_bin_sdk.generated.json
```

The Rust test reads that file when present. If the file is absent, the offline test exits without requiring Node, the SDK, or network access.

The runtime quote helper can also be invoked directly:

```powershell
npm run quote -- --pool <LB_PAIR_ADDRESS> --trade-size-usdc 100 --bin-array-count 4 --slippage-bps 50
```

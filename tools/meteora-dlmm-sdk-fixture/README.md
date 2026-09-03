# Meteora-DLMM Active Bin SDK Fixture

This helper generates the local SDK reference fixture used by the Rust offline test for the Meteora-DLMM active bin price formula.

The generated fixture is intentionally not committed. It depends on the live RPC state at generation time and on the installed `@meteora-ag/dlmm` version.

## Usage

```powershell
cd tools/meteora-dlmm-sdk-fixture
npm install
$env:HELIUS_RPC_URL = "https://mainnet.helius-rpc.com/?api-key=..."
npm run generate -- --pool <LB_PAIR_ADDRESS_1> --pool <LB_PAIR_ADDRESS_2>
```

By default, the output is written to:

```text
../../tests/fixtures/meteora_dlmm_active_bin_sdk.generated.json
```

The Rust test reads that file when present. If the file is absent, the offline test exits without requiring Node, the SDK, or network access.

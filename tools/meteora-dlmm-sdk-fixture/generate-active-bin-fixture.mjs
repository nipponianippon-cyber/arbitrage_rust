import { createRequire } from "node:module";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import Decimal from "decimal.js";
import DLMM from "@meteora-ag/dlmm";
import { Connection, PublicKey } from "@solana/web3.js";

const require = createRequire(import.meta.url);
const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));

const DEFAULT_OUTPUT =
  "../../tests/fixtures/meteora_dlmm_active_bin_sdk.generated.json";
const DEFAULT_CLUSTER = "mainnet-beta";
const DEFAULT_BASE_MINT = "So11111111111111111111111111111111111111112";
const DEFAULT_QUOTE_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

function readPackageVersion(packageName) {
  try {
    return require(`${packageName}/package.json`).version ?? "unknown";
  } catch {
    return "unknown";
  }
}

function parseArgs(argv) {
  const options = {
    pools: [],
    output: DEFAULT_OUTPUT,
    cluster: process.env.METEORA_CLUSTER ?? DEFAULT_CLUSTER,
    rpcUrl: process.env.HELIUS_RPC_URL ?? process.env.RPC_URL,
    baseMint: process.env.BASE_MINT ?? DEFAULT_BASE_MINT,
    quoteMint: process.env.QUOTE_MINT ?? DEFAULT_QUOTE_MINT,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];

    if (arg === "--pool" && next) {
      options.pools.push(next);
      index += 1;
    } else if (arg === "--output" && next) {
      options.output = next;
      index += 1;
    } else if (arg === "--cluster" && next) {
      options.cluster = next;
      index += 1;
    } else if (arg === "--rpc-url" && next) {
      options.rpcUrl = next;
      index += 1;
    } else if (arg === "--base-mint" && next) {
      options.baseMint = next;
      index += 1;
    } else if (arg === "--quote-mint" && next) {
      options.quoteMint = next;
      index += 1;
    } else {
      throw new Error(`unsupported argument: ${arg}`);
    }
  }

  if (!options.rpcUrl) {
    throw new Error("set HELIUS_RPC_URL, RPC_URL, or pass --rpc-url");
  }
  if (options.pools.length === 0) {
    throw new Error("pass at least one --pool <LB_PAIR_ADDRESS>");
  }

  return options;
}

function publicKeyString(value) {
  if (value?.toBase58) {
    return value.toBase58();
  }
  if (value?.address?.toBase58) {
    return value.address.toBase58();
  }
  if (typeof value?.address === "string") {
    return value.address;
  }
  return String(value);
}

function tokenMintAddress(token) {
  return publicKeyString(token?.mint?.address ?? token?.mint ?? token?.publicKey);
}

function tokenMintDecimals(token) {
  const decimals = token?.mint?.decimals ?? token?.decimal ?? token?.decimals;
  if (!Number.isInteger(decimals)) {
    throw new Error("SDK token mint decimals were not available");
  }
  return decimals;
}

function numberFrom(value, fieldName) {
  const numberValue = Number(value?.toString ? value.toString() : value);
  if (!Number.isFinite(numberValue)) {
    throw new Error(`SDK ${fieldName} was not numeric`);
  }
  return numberValue;
}

function normalizedUsdcPerSol({ uiPrice, tokenXMint, tokenYMint, baseMint, quoteMint }) {
  const price = new Decimal(uiPrice);
  if (tokenXMint === baseMint && tokenYMint === quoteMint) {
    return price.toString();
  }
  if (tokenXMint === quoteMint && tokenYMint === baseMint) {
    return new Decimal(1).div(price).toString();
  }
  throw new Error(
    `pool token mints do not match configured base/quote: ${tokenXMint}, ${tokenYMint}`,
  );
}

async function fixtureForPool(connection, options, poolAddress) {
  // SDKの同一読み取り結果から、Rust側が価格式だけを再計算するための入力と期待値を固定する。
  const dlmm = await DLMM.create(connection, new PublicKey(poolAddress), {
    cluster: options.cluster,
  });
  const activeBin = await dlmm.getActiveBin();
  const pricePerLamport = activeBin.price.toString();
  const sdkUiPrice = dlmm.fromPricePerLamport(Number(activeBin.price)).toString();
  const tokenXMint = tokenMintAddress(dlmm.tokenX);
  const tokenYMint = tokenMintAddress(dlmm.tokenY);

  return {
    lb_pair_address: poolAddress,
    active_id: numberFrom(activeBin.binId, "active bin id"),
    bin_step: numberFrom(dlmm.lbPair.binStep, "bin step"),
    token_x_mint: tokenXMint,
    token_y_mint: tokenYMint,
    token_x_decimals: tokenMintDecimals(dlmm.tokenX),
    token_y_decimals: tokenMintDecimals(dlmm.tokenY),
    sdk_price_per_lamport: pricePerLamport,
    sdk_ui_price: sdkUiPrice,
    normalized_usdc_per_sol: normalizedUsdcPerSol({
      uiPrice: sdkUiPrice,
      tokenXMint,
      tokenYMint,
      baseMint: options.baseMint,
      quoteMint: options.quoteMint,
    }),
  };
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const connection = new Connection(options.rpcUrl, "confirmed");
  const outputPath = resolve(SCRIPT_DIR, options.output);

  const fixtures = [];
  for (const poolAddress of options.pools) {
    fixtures.push(await fixtureForPool(connection, options, poolAddress));
  }

  const body = {
    schema_version: 1,
    source: "meteora_dlmm_sdk",
    sdk_package: "@meteora-ag/dlmm",
    sdk_version: readPackageVersion("@meteora-ag/dlmm"),
    generated_at: new Date().toISOString(),
    cluster: options.cluster,
    base_mint: options.baseMint,
    quote_mint: options.quoteMint,
    fixtures,
  };

  await mkdir(dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(body, null, 2)}\n`);
  console.log(`wrote ${outputPath}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

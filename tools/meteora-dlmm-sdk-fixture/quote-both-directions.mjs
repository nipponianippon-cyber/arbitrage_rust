import { createRequire } from "node:module";
import Decimal from "decimal.js";
import DLMM from "@meteora-ag/dlmm";
import { BN } from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";

const require = createRequire(import.meta.url);
const DEFAULT_CLUSTER = "mainnet-beta";
const DEFAULT_BASE_MINT = "So11111111111111111111111111111111111111112";
const DEFAULT_QUOTE_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

function parseArgs(argv) {
  const options = {
    pool: undefined,
    cluster: process.env.METEORA_CLUSTER ?? DEFAULT_CLUSTER,
    rpcUrl: process.env.HELIUS_RPC_URL ?? process.env.RPC_URL,
    baseMint: process.env.BASE_MINT ?? DEFAULT_BASE_MINT,
    quoteMint: process.env.QUOTE_MINT ?? DEFAULT_QUOTE_MINT,
    tradeSizeUsdc: process.env.TRADE_SIZE_USDC ?? "100",
    binArrayCount: Number(process.env.METEORA_DLMM_BIN_ARRAY_COUNT ?? "4"),
    slippageBps: Number(process.env.METEORA_DLMM_SLIPPAGE_BPS ?? "50"),
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];

    if (arg === "--pool" && next) {
      options.pool = next;
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
    } else if (arg === "--trade-size-usdc" && next) {
      options.tradeSizeUsdc = next;
      index += 1;
    } else if (arg === "--bin-array-count" && next) {
      options.binArrayCount = Number(next);
      index += 1;
    } else if (arg === "--slippage-bps" && next) {
      options.slippageBps = Number(next);
      index += 1;
    } else {
      throw new Error(`unsupported argument: ${arg}`);
    }
  }

  if (!options.pool) {
    throw new Error("pass --pool <LB_PAIR_ADDRESS>");
  }
  if (!options.rpcUrl) {
    throw new Error("set HELIUS_RPC_URL, RPC_URL, or pass --rpc-url");
  }
  if (!Number.isInteger(options.binArrayCount) || options.binArrayCount <= 0) {
    throw new Error("--bin-array-count must be a positive integer");
  }
  if (!Number.isInteger(options.slippageBps) || options.slippageBps < 0) {
    throw new Error("--slippage-bps must be a non-negative integer");
  }
  if (new Decimal(options.tradeSizeUsdc).lte(0)) {
    throw new Error("--trade-size-usdc must be positive");
  }

  return options;
}

function readPackageVersion(packageName) {
  try {
    return require(`${packageName}/package.json`).version ?? "unknown";
  } catch {
    return "unknown";
  }
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

function rawToUi(rawAmount, decimals) {
  return new Decimal(rawAmount.toString()).div(new Decimal(10).pow(decimals));
}

function uiToRaw(uiAmount, decimals) {
  return new Decimal(uiAmount).mul(new Decimal(10).pow(decimals)).floor().toFixed(0);
}

function normalizedUsdcPerSol({ uiPrice, tokenXMint, tokenYMint, baseMint, quoteMint }) {
  const price = new Decimal(uiPrice);
  if (tokenXMint === baseMint && tokenYMint === quoteMint) {
    return price;
  }
  if (tokenXMint === quoteMint && tokenYMint === baseMint) {
    return new Decimal(1).div(price);
  }
  throw new Error(
    `pool token mints do not match configured base/quote: ${tokenXMint}, ${tokenYMint}`,
  );
}

function swapForDirection(direction, tokenXMint, tokenYMint, baseMint, quoteMint) {
  if (direction === "USDC -> SOL") {
    if (tokenXMint === quoteMint && tokenYMint === baseMint) return true;
    if (tokenYMint === quoteMint && tokenXMint === baseMint) return false;
  }
  if (direction === "SOL -> USDC") {
    if (tokenXMint === baseMint && tokenYMint === quoteMint) return true;
    if (tokenYMint === baseMint && tokenXMint === quoteMint) return false;
  }
  throw new Error(`cannot map ${direction} to token X/Y direction`);
}

function quoteFailure({
  direction,
  inputMint,
  outputMint,
  requestedInputAmount,
  requestedInputAmountRaw,
  error,
}) {
  return {
    direction,
    input_mint: inputMint,
    output_mint: outputMint,
    requested_input_amount: requestedInputAmount.toString(),
    requested_input_amount_raw: requestedInputAmountRaw,
    consumed_input_amount: null,
    consumed_input_amount_raw: null,
    output_amount: null,
    output_amount_raw: null,
    fee_amount: null,
    fee_amount_raw: null,
    protocol_fee_amount: null,
    protocol_fee_amount_raw: null,
    price_impact_bps: null,
    effective_price: null,
    end_price: null,
    bin_array_addresses: [],
    partial_fill: false,
    success: false,
    error_message: error?.message ?? String(error),
  };
}

async function quoteDirection({ dlmm, direction, options, tokenXMint, tokenYMint, normalizedPrice }) {
  const tokenXDecimals = tokenMintDecimals(dlmm.tokenX);
  const tokenYDecimals = tokenMintDecimals(dlmm.tokenY);
  const swapForY = swapForDirection(
    direction,
    tokenXMint,
    tokenYMint,
    options.baseMint,
    options.quoteMint,
  );
  const inputMint = direction === "USDC -> SOL" ? options.quoteMint : options.baseMint;
  const outputMint = direction === "USDC -> SOL" ? options.baseMint : options.quoteMint;
  const inputDecimals = inputMint === tokenXMint ? tokenXDecimals : tokenYDecimals;
  const outputDecimals = outputMint === tokenXMint ? tokenXDecimals : tokenYDecimals;
  const requestedInputAmount =
    direction === "USDC -> SOL"
      ? new Decimal(options.tradeSizeUsdc)
      : new Decimal(options.tradeSizeUsdc).div(normalizedPrice);
  const requestedInputAmountRaw = uiToRaw(requestedInputAmount, inputDecimals);

  try {
    const binArrays = await dlmm.getBinArrayForSwap(swapForY, options.binArrayCount);
    const quote = dlmm.swapQuote(
      new BN(requestedInputAmountRaw),
      swapForY,
      new BN(options.slippageBps),
      binArrays,
      false,
    );
    const consumedInputAmountRaw = quote.consumedInAmount.toString();
    const outputAmountRaw = quote.outAmount.toString();
    const consumedInputAmount = rawToUi(consumedInputAmountRaw, inputDecimals);
    const outputAmount = rawToUi(outputAmountRaw, outputDecimals);
    if (consumedInputAmount.lte(0) || outputAmount.lte(0)) {
      throw new Error("SDK quote returned a non-positive consumed input or output amount");
    }
    const feeAmount = rawToUi(quote.fee.toString(), inputDecimals);
    const protocolFeeAmount = rawToUi(quote.protocolFee.toString(), inputDecimals);
    const effectivePrice =
      direction === "USDC -> SOL"
        ? consumedInputAmount.div(outputAmount)
        : outputAmount.div(consumedInputAmount);

    return {
      direction,
      input_mint: inputMint,
      output_mint: outputMint,
      requested_input_amount: requestedInputAmount.toString(),
      requested_input_amount_raw: requestedInputAmountRaw,
      consumed_input_amount: consumedInputAmount.toString(),
      consumed_input_amount_raw: consumedInputAmountRaw,
      output_amount: outputAmount.toString(),
      output_amount_raw: outputAmountRaw,
      fee_amount: feeAmount.toString(),
      fee_amount_raw: quote.fee.toString(),
      protocol_fee_amount: protocolFeeAmount.toString(),
      protocol_fee_amount_raw: quote.protocolFee.toString(),
      price_impact_bps: new Decimal(quote.priceImpact.toString()).mul(100).toString(),
      effective_price: effectivePrice.toString(),
      end_price: quote.endPrice?.toString?.() ?? null,
      bin_array_addresses: (quote.binArraysPubkey ?? binArrays.map((item) => item.publicKey)).map(
        publicKeyString,
      ),
      partial_fill: consumedInputAmountRaw !== requestedInputAmountRaw,
      success: true,
      error_message: null,
    };
  } catch (error) {
    return quoteFailure({
      direction,
      inputMint,
      outputMint,
      requestedInputAmount,
      requestedInputAmountRaw,
      error,
    });
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const connection = new Connection(options.rpcUrl, "confirmed");
  const dlmm = await DLMM.create(connection, new PublicKey(options.pool), {
    cluster: options.cluster,
  });
  const activeBin = await dlmm.getActiveBin();
  const tokenXMint = tokenMintAddress(dlmm.tokenX);
  const tokenYMint = tokenMintAddress(dlmm.tokenY);
  const sdkUiPrice = dlmm.fromPricePerLamport(Number(activeBin.price)).toString();
  const normalizedPrice = normalizedUsdcPerSol({
    uiPrice: sdkUiPrice,
    tokenXMint,
    tokenYMint,
    baseMint: options.baseMint,
    quoteMint: options.quoteMint,
  });
  const quotes = [];
  for (const direction of ["USDC -> SOL", "SOL -> USDC"]) {
    quotes.push(
      await quoteDirection({
        dlmm,
        direction,
        options,
        tokenXMint,
        tokenYMint,
        normalizedPrice,
      }),
    );
  }

  const body = {
    schema_version: 1,
    source: "meteora_dlmm_sdk",
    sdk_package: "@meteora-ag/dlmm",
    sdk_version: readPackageVersion("@meteora-ag/dlmm"),
    generated_at: new Date().toISOString(),
    cluster: options.cluster,
    lb_pair_address: options.pool,
    base_mint: options.baseMint,
    quote_mint: options.quoteMint,
    trade_size_usdc: options.tradeSizeUsdc,
    bin_array_count: options.binArrayCount,
    slippage_bps: options.slippageBps,
    quotes,
  };
  process.stdout.write(`${JSON.stringify(body, null, 2)}\n`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

// from: shop@94584a5c:app/src/lib/names.ts, marketplace@f3956826:webapp/src/modules/ens/{sagas,utils}.ts,
// catalyrst-v2 sites/src/data/lib/catalyst/marketplace/name-claim.ts (the registrar flow v2 runs today)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace sources). Licensed under the Apache License, Version 2.0.
// Primary NAME registration. SHOP.md: credits are the displayed price and the main action; a web3
// wallet also sees direct MANA registration through the registrar controller on Ethereum, second.
// A NAME resale is a different sale (MANA only) and never comes through here.
//
// Credits (CLAIM_NAME_WITH_CREDITS_REQUEST, upstream `registerNameWithUsdCredits`): reserve one credit
// for the NAME's credits quote, fetch the server-built cross-chain route (`/shop-api/v1/names/credit-route`),
// submit `CreditsManager.useCredits` on Polygon with the route as its external call, then follow the
// Across fill until the NAME is minted on Ethereum. The credit is released only while the origin
// transaction is known not to have been sent; once it may have been, the reservation is kept and the
// server's reconciler settles it. The server refuses the route while NAME credits are closed
// (docs/shop/CONTRACTS.md "NAME credits"), which the modal shows as the credits option being unavailable.
//
// MANA (CLAIM_NAME_REQUEST, upstream `registerNameWithEthereumMana`): the buyer's own wallet approves
// 100 MANA to DCLControllerV2 and calls `register(name, buyer)` on Ethereum, paying the gas itself.

import { keccak256, parseAbi, stringToHex, type Hex } from "viem";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { isShopApiError } from "../api/client";
import { shopClient, type ShopClient } from "../api/shop";
import type { AuthorizedCredit, CreditAuthorization } from "../api/types";
import { CHAIN, type ChainId } from "../model/chain";
import { manaUsdCentsCeil, type ManaUsdRate, type ManaWei, type UsdCents } from "../model/money";
import { requireChain } from "../wallet/chain";
import { requireContracts } from "../checkout/chain/contracts";
import { isGaslessUnavailable, relayMetaTransaction } from "../checkout/chain/metatx";
import { shopChainReader, type ChainReader as ShopChainReader } from "../checkout/chain/rpc";
import { landChainId, landNetwork } from "../land/contracts";
import {
  encodeCall,
  ensureAllowance,
  erc20Abi,
  readContract,
  rpcReader,
  sendTransaction,
  TxPendingError,
  waitForReceipt,
  walletReader,
  type ChainReader,
} from "../land/evm";

/** 100 MANA: DCLControllerV2's `PRICE` and the useCredits `maxCreditedValue` of a NAME. */
export const NAME_PRICE_IN_WEI = "100000000000000000000";
export const NAME_PRICE_WEI = 100n * 10n ** 18n;

export const NAME_MIN_LENGTH = 2;
export const NAME_MAX_LENGTH = 15;
const NAME_ALLOWED = /^[a-zA-Z0-9]+$/;

export type NameInvalidReason = "empty" | "too-short" | "too-long" | "invalid-chars" | "has-spaces";
export type NameValidation = { ok: true } | { ok: false; reason: NameInvalidReason };

/**
 * Validates a typed NAME: 2-15 ASCII letters or digits (`src/shop/types.rs` NameLabel::parse). A
 * space is reported apart from other characters, as the marketplace's claim page did.
 */
export function validateName(raw: string): NameValidation {
  const name = raw.trim();
  if (name.length === 0) return { ok: false, reason: "empty" };
  if (/\s/.test(name)) return { ok: false, reason: "has-spaces" };
  if (!NAME_ALLOWED.test(name)) return { ok: false, reason: "invalid-chars" };
  if (name.length < NAME_MIN_LENGTH) return { ok: false, reason: "too-short" };
  if (name.length > NAME_MAX_LENGTH) return { ok: false, reason: "too-long" };
  return { ok: true };
}

export const NAME_INVALID_TEXT: Record<Exclude<NameInvalidReason, "empty">, string> = {
  "too-short": "NAME too short: NAMEs must be at least 2 characters long.",
  "too-long": "NAMEs can contain up to 15 characters.",
  "invalid-chars": "NAMEs can only contain alphanumeric characters.",
  "has-spaces": "NAMEs can't contain spaces.",
};

/** Keeps a typed value registrable: drops anything but letters and digits and caps the length. */
export function sanitizeNameInput(raw: string): string {
  return raw.replace(/[^a-zA-Z0-9]/g, "").slice(0, NAME_MAX_LENGTH);
}

/** The registrar token of a NAME: `uint256(keccak256(lower(label)))`, decimal. */
export function nameTokenIdOf(name: string): string {
  return BigInt(keccak256(stringToHex(name.toLowerCase()))).toString();
}

export const registrarAbi = parseAbi([
  "function available(string _subdomain) view returns (bool)",
  "function ownerOf(uint256 tokenId) view returns (address)",
]);

export const controllerAbi = parseAbi([
  "function PRICE() view returns (uint256)",
  "function acceptedToken() view returns (address)",
  "function registrar() view returns (address)",
  "function register(string _name, address _beneficiary)",
]);

export type NameAvailability = "available" | "taken";

/**
 * `DCLRegistrar.available(name)` on the Ethereum network the Shop registers on: the gate the
 * registration itself enforces. Advisory at search time; the server and the chain decide at purchase.
 */
export async function checkNameAvailability(
  name: string,
  opts: { signal?: AbortSignal; reader?: ChainReader; chainId?: ChainId } = {},
): Promise<NameAvailability> {
  const chainId = opts.chainId ?? landChainId();
  const reader = opts.reader ?? rpcReader(chainId);
  const available = await readContract<boolean>(reader, landNetwork(chainId).nameRegistrar, registrarAbi, "available", [name], opts.signal);
  if (opts.signal?.aborted) throw new DOMException("Aborted", "AbortError");
  return available ? "available" : "taken";
}

/** USD cents a NAME costs at `rate`: 100 MANA, rounded up (upstream `sizeNameUsdCents`). */
export function sizeNameUsdCents(rate: ManaUsdRate): UsdCents | null {
  return manaUsdCentsCeil(NAME_PRICE_WEI as ManaWei, rate);
}

export type NameRouteProvider = "axelar" | "across";

export type NameRouteExternalCall = { target: string; selector: string; data: string; expiresAt: number; salt: string };

/** `GET /shop-api/v1/names/credit-route`: the server-built, server-signed route (legacy `/credits/credits-name-route`). */
export type NameCreditRoute = {
  externalCall: NameRouteExternalCall;
  customExternalCallSignature: string;
  quoteId: string;
  estimatedRouteDuration: number;
  fromChainId: string;
  toChainId: string;
  provider?: NameRouteProvider;
};

export class NameRouteCostTooHighError extends Error {
  override name = "NameRouteCostTooHighError";
  constructor() {
    super("Name registration is temporarily unavailable due to high network costs");
  }
}

/** The server does not register NAMEs with credits right now (upstream 404 `NAMES_DISABLED`). */
export class NameCreditsClosedError extends Error {
  override name = "NameCreditsClosedError";
  constructor() {
    super("Registering a NAME with Credits is not available right now.");
  }
}

/** The credit was consumed but the NAME was not minted; the bridged MANA went to recovery. Never retry. */
export class NameNotRegisteredError extends Error {
  override name = "NameNotRegisteredError";
  constructor() {
    super("The name could not be registered on Ethereum; your funds were recovered.");
  }
}

/** Whether the credit was spent is unknown: a retry could double-spend. */
export class NameSettlementUnknownError extends Error {
  override name = "NameSettlementUnknownError";
  constructor() {
    super("We couldn't confirm whether the purchase went through.");
  }
}

/** The Polygon chain the credits of an Ethereum NAME network settle on. */
export function creditsChainFor(ethereumChainId: ChainId): ChainId {
  return ethereumChainId === CHAIN.SEPOLIA ? CHAIN.AMOY : CHAIN.POLYGON;
}

export async function fetchNameCreditRoute(
  name: string,
  opts: { chainId?: ChainId; provider?: NameRouteProvider; client?: ShopClient; signal?: AbortSignal } = {},
): Promise<NameCreditRoute> {
  const client = opts.client ?? shopClient();
  const chainId = opts.chainId ?? creditsChainFor(landChainId());
  try {
    return await client.credits.nameRoute<NameCreditRoute>({ name, chainId, provider: opts.provider ?? "across" }, { signal: opts.signal });
  } catch (e) {
    const code = isShopApiError(e) ? (e.body as { code?: unknown } | null)?.code : undefined;
    if (code === "ROUTE_COST_TOO_HIGH") throw new NameRouteCostTooHighError();
    if (code === "NAMES_DISABLED") throw new NameCreditsClosedError();
    throw e;
  }
}

/** A credit id as the bytes32 salt `useCredits` takes (upstream `idToSalt`). */
export function idToSalt(id: string): Hex {
  if (!id) return `0x${"00".repeat(32)}`;
  const hex = id.startsWith("0x") ? id.slice(2) : Array.from(new TextEncoder().encode(id), (b) => b.toString(16).padStart(2, "0")).join("");
  return `0x${hex.padStart(64, "0")}`;
}

export type NameUseCreditsArgs = {
  credits: { value: string; expiresAt: number; salt: Hex }[];
  creditsSignatures: string[];
  externalCall: NameRouteExternalCall;
  customExternalCallSignature: string;
  maxUncreditedValue: string;
  maxCreditedValue: string;
};

/**
 * useCredits arguments for a NAME: the credit pays, the server-signed route IS the operation,
 * `maxCreditedValue` is the 100 MANA price and `maxUncreditedValue` whatever the credit cannot cover.
 */
export function buildNameUseCreditsArgs(credit: AuthorizedCredit, route: NameCreditRoute): NameUseCreditsArgs {
  const uncredited = NAME_PRICE_WEI - BigInt(credit.availableAmount);
  return {
    credits: [{ value: credit.amount, expiresAt: Number(credit.expiresAt), salt: idToSalt(credit.id) }],
    creditsSignatures: [credit.signature],
    externalCall: { ...route.externalCall },
    customExternalCallSignature: route.customExternalCallSignature,
    maxUncreditedValue: uncredited < 0n ? "0" : uncredited.toString(),
    maxCreditedValue: NAME_PRICE_IN_WEI,
  };
}

export const useCreditsAbi = [
  {
    type: "function",
    name: "useCredits",
    stateMutability: "nonpayable",
    inputs: [
      {
        name: "_args",
        type: "tuple",
        components: [
          {
            name: "credits",
            type: "tuple[]",
            components: [
              { name: "value", type: "uint256" },
              { name: "expiresAt", type: "uint256" },
              { name: "salt", type: "bytes32" },
            ],
          },
          { name: "creditsSignatures", type: "bytes[]" },
          {
            name: "externalCall",
            type: "tuple",
            components: [
              { name: "target", type: "address" },
              { name: "selector", type: "bytes4" },
              { name: "data", type: "bytes" },
              { name: "expiresAt", type: "uint256" },
              { name: "salt", type: "bytes32" },
            ],
          },
          { name: "customExternalCallSignature", type: "bytes" },
          { name: "maxUncreditedValue", type: "uint256" },
          { name: "maxCreditedValue", type: "uint256" },
        ],
      },
    ],
    outputs: [],
  },
] as const;

export function encodeUseCredits(args: NameUseCreditsArgs): Hex {
  return encodeCall(useCreditsAbi, "useCredits", [
    {
      credits: args.credits.map((c) => ({ value: BigInt(c.value), expiresAt: BigInt(c.expiresAt), salt: c.salt })),
      creditsSignatures: args.creditsSignatures,
      externalCall: {
        target: args.externalCall.target,
        selector: args.externalCall.selector,
        data: args.externalCall.data,
        expiresAt: BigInt(args.externalCall.expiresAt),
        salt: idToSalt(args.externalCall.salt),
      },
      customExternalCallSignature: args.customExternalCallSignature,
      maxUncreditedValue: BigInt(args.maxUncreditedValue),
      maxCreditedValue: BigInt(args.maxCreditedValue),
    },
  ]);
}

export type AcrossNameStatus = {
  status: "pending" | "filled" | "refunded" | "expired";
  destinationTxHash: string | null;
  /** null: Across did not report it, which is not the same as false. */
  actionsSucceeded: boolean | null;
};

const ACROSS_API_URL = "https://app.across.to/api";

/** Polls Across until the deposit is terminal or the attempts run out (returns the last `pending`). */
export async function pollAcrossNameStatus(
  originTxHash: string,
  opts: { intervalMs?: number; maxAttempts?: number; fetchImpl?: typeof fetch; apiUrl?: string; originChainId?: number } = {},
): Promise<AcrossNameStatus> {
  const doFetch = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
  const interval = opts.intervalMs ?? 10_000;
  const attempts = opts.maxAttempts ?? 60;
  let last: AcrossNameStatus = { status: "pending", destinationTxHash: null, actionsSucceeded: null };
  for (let i = 0; i < attempts; i++) {
    try {
      const res = await doFetch(`${opts.apiUrl ?? ACROSS_API_URL}/deposit/status?originChainId=${opts.originChainId ?? 137}&depositTxHash=${originTxHash}`);
      if (res.ok) {
        const data = (await res.json()) as { status?: string; fillTx?: string; fillTxnRef?: string; actionsSucceeded?: boolean };
        const status = (data.status ?? "pending").toLowerCase();
        const destinationTxHash = data.fillTx ?? data.fillTxnRef ?? null;
        const actionsSucceeded = typeof data.actionsSucceeded === "boolean" ? data.actionsSucceeded : null;
        if (status === "filled") return { status: "filled", destinationTxHash, actionsSucceeded };
        if (status === "refunded" || status === "expired") return { status, destinationTxHash: null, actionsSucceeded: false };
        last = { status: "pending", destinationTxHash, actionsSucceeded };
      }
    } catch {
      // Not indexed yet or a transient failure: back off and retry.
    }
    if (i < attempts - 1) await new Promise((resolve) => setTimeout(resolve, interval));
  }
  return last;
}

/** What the screen watching a registration says; the phases last very different times. */
export type NameRegistrationStage = "preparing" | "awaiting-confirmation" | "confirming" | "registering";

export type NameRegistrationResult =
  | { status: "registered"; originTxHash: string; destinationTxHash: string | null }
  | { status: "pending"; originTxHash: string };

/** Submits the useCredits call; the default is the buyer's own wallet on Polygon (the buyer pays gas). */
export type UseCreditsSubmitter = (req: { chainId: ChainId; manager: string; buyer: string; args: NameUseCreditsArgs }) => Promise<string>;

export function walletUseCreditsSubmitter(provider: Eip1193Provider): UseCreditsSubmitter {
  return ({ chainId, manager, buyer, args }) => sendTransaction(provider, { chainId, from: buyer, to: manager, data: encodeUseCredits(args) });
}

/**
 * A managed (in-app) wallet holds no gas: its useCredits is signed as a meta-transaction on the
 * CreditsManager and relayed. A relay that gave no usable answer is reported as unobservable, never
 * as a failure the reservation could be released on.
 */
export function relayedUseCreditsSubmitter(
  provider: Eip1193Provider,
  opts: { reader?: ShopChainReader; relayerUrl?: string; fetchImpl?: typeof fetch } = {},
): UseCreditsSubmitter {
  return async ({ chainId, manager, buyer, args }) => {
    const known = requireContracts(chainId).creditsManager;
    if (!known || known.address.toLowerCase() !== manager.toLowerCase()) throw new Error("This credit names a Credits manager the Shop does not know.");
    try {
      const { txHash } = await relayMetaTransaction({
        provider,
        reader: opts.reader ?? shopChainReader(),
        target: { address: known.address, chainId, name: known.name, version: known.version, metaTx: "functionData" },
        from: buyer,
        functionData: encodeUseCredits(args),
        relayerUrl: opts.relayerUrl,
        fetchImpl: opts.fetchImpl,
      });
      return txHash;
    } catch (e) {
      if (isGaslessUnavailable(e) && e.reason === "relayer-unreachable") {
        const unreachable = new Error(e.message);
        unreachable.name = "RelayerUnreachableError";
        throw unreachable;
      }
      throw e;
    }
  };
}

const POLYGON_MANA: Record<number, string> = {
  [CHAIN.POLYGON]: "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4",
  [CHAIN.AMOY]: "0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0",
};

export type RegisterWithCreditsInput = {
  name: string;
  /** The signed-in account: payer and beneficiary (the route registers to the signed-fetch identity). */
  buyer: string;
  /** The credits leg in cents: the NAME's credits quote, or less on the mixed rail (never zero). */
  creditsCents: UsdCents;
  /** The full credits quote, to tell the credits-only purchase from the mixed rail. */
  quotedCents: UsdCents;
  /** The most MANA the mixed rail may pull, as shown to the buyer. */
  maxManaWei?: bigint;
  provider: Eip1193Provider;
  client?: ShopClient;
  submit?: UseCreditsSubmitter;
  chainId?: ChainId;
  route?: NameRouteProvider;
  acrossPoll?: { intervalMs?: number; maxAttempts?: number; fetchImpl?: typeof fetch };
  wait?: Parameters<typeof waitForReceipt>[2];
  onProgress?(stage: NameRegistrationStage): void;
};

/**
 * CLAIM_NAME_WITH_CREDITS_REQUEST. Releases the reservation only while the origin transaction is
 * known unsent; returns `pending` when the origin transaction or the bridge has not finished within
 * the wait (the reservation stays, the server's reconciler settles it).
 */
export async function registerNameWithUsdCredits(input: RegisterWithCreditsInput): Promise<NameRegistrationResult> {
  const progress = (stage: NameRegistrationStage) => {
    try {
      input.onProgress?.(stage);
    } catch {
      // Reporting progress must not abort a purchase.
    }
  };
  const client = input.client ?? shopClient();
  const chainId = input.chainId ?? creditsChainFor(landChainId());
  const buyer = input.buyer.toLowerCase();
  const payingWithMana = input.creditsCents < input.quotedCents;
  if (input.creditsCents <= 0) throw new Error("A NAME is paid with at least one Credit.");
  let salt: string | null = null;
  let origin: "unsent" | "sent" | "unobservable" = "unsent";
  progress("preparing");
  try {
    const route = await fetchNameCreditRoute(input.name, { chainId, provider: input.route ?? "across", client });
    const authorized: CreditAuthorization = await client.authorizations.authorize({ usdPriceCents: input.creditsCents, name: input.name, source: "website" });
    salt = authorized.credit.id;
    if (!payingWithMana && BigInt(authorized.maxCreditedValue) < NAME_PRICE_WEI) throw new Error("Credit under-sized for the name price");
    if (payingWithMana) {
      const gap = NAME_PRICE_WEI - BigInt(authorized.credit.availableAmount);
      if (input.maxManaWei != null && gap > input.maxManaWei) throw new Error("The MANA needed moved past what was agreed");
      if (gap > 0n) {
        progress("awaiting-confirmation");
        await ensureAllowance(input.provider, {
          chainId,
          token: POLYGON_MANA[chainId] ?? POLYGON_MANA[CHAIN.POLYGON]!,
          owner: buyer,
          spender: authorized.credit.contract,
          required: gap,
          wait: input.wait,
        });
      }
    }
    progress("awaiting-confirmation");
    const args = buildNameUseCreditsArgs(authorized.credit, route);
    const submit = input.submit ?? walletUseCreditsSubmitter(input.provider);
    let originTxHash: string;
    try {
      originTxHash = await submit({ chainId, manager: authorized.credit.contract, buyer, args });
    } catch (e) {
      if ((e as { name?: string } | null)?.name === "RelayerUnreachableError") {
        origin = "unobservable";
        const unknown: Error & { cause?: unknown } = new NameSettlementUnknownError();
        unknown.cause = e;
        throw unknown;
      }
      throw e;
    }
    origin = "sent";
    client.authorizations.submitted({ salts: [salt], txHash: originTxHash }).catch(() => {});
    progress("confirming");
    try {
      await waitForReceipt(input.provider, originTxHash, input.wait);
    } catch (e) {
      if (e instanceof TxPendingError) return { status: "pending", originTxHash };
      throw e;
    }
    if ((input.route ?? "across") !== "across") return { status: "pending", originTxHash };
    progress("registering");
    const across = await pollAcrossNameStatus(originTxHash, { ...input.acrossPoll, originChainId: chainId });
    if (across.status === "pending") return { status: "pending", originTxHash };
    if (across.status === "filled" && across.actionsSucceeded === null) return { status: "pending", originTxHash };
    if (across.status !== "filled" || across.actionsSucceeded !== true) throw new NameNotRegisteredError();
    return { status: "registered", originTxHash, destinationTxHash: across.destinationTxHash };
  } catch (e) {
    if (salt && origin === "unsent") await client.authorizations.cancel({ salts: [salt] }).catch(() => {});
    throw e;
  }
}

// ---- direct MANA registration on Ethereum ----

const PENDING_KEY = (account: string, name: string) => `dcl-shop-name-register:v1:${account.toLowerCase()}:${name.toLowerCase()}`;

function pendingStore(): Pick<Storage, "getItem" | "setItem" | "removeItem"> | null {
  try {
    return typeof sessionStorage !== "undefined" ? sessionStorage : null;
  } catch {
    return null;
  }
}

/** A registration already handed to the wallet for this account and NAME, so a retry follows it instead of paying twice. */
export function pendingNameRegistration(account: string, name: string): string | null {
  try {
    const hash = pendingStore()?.getItem(PENDING_KEY(account, name)) ?? null;
    return hash && /^0x[0-9a-fA-F]{64}$/.test(hash) ? hash : null;
  } catch {
    return null;
  }
}

function savePending(account: string, name: string, hash: string | null) {
  try {
    if (hash) pendingStore()?.setItem(PENDING_KEY(account, name), hash);
    else pendingStore()?.removeItem(PENDING_KEY(account, name));
  } catch {
    // Without storage a retry re-checks availability, which catches a registration that landed.
  }
}

export class NameControllerChangedError extends Error {
  override name = "NameControllerChangedError";
  constructor() {
    super("The NAME controller configuration has changed. Registration is unavailable.");
  }
}

export class NameTakenError extends Error {
  override name = "NameTakenError";
  constructor() {
    super("This NAME has already been registered. Choose another.");
  }
}

export class InsufficientManaError extends Error {
  override name = "InsufficientManaError";
  constructor(
    readonly balance: bigint,
    readonly required: bigint,
  ) {
    super("Your wallet does not hold enough MANA on Ethereum to register this NAME.");
  }
}

export type RegisterWithManaInput = {
  name: string;
  buyer: string;
  provider: Eip1193Provider;
  /** Only a web3 wallet session can pay Ethereum gas itself (upstream `canPayGasItself`). */
  sessionKind: "web3" | "web2" | "signedOut";
  chainId?: ChainId;
  reader?: ChainReader;
  wait?: Parameters<typeof waitForReceipt>[2];
  onProgress?(stage: NameRegistrationStage | "approving"): void;
};

/**
 * CLAIM_NAME_REQUEST: register with the buyer's MANA on Ethereum. Verifies the controller still
 * charges `PRICE` in MANA for the configured registrar, the NAME is free, the balance covers it, then
 * approves (only when short) and registers, and checks the registrar now names the buyer.
 */
export async function registerNameWithEthereumMana(input: RegisterWithManaInput): Promise<{ status: "registered"; originTxHash: string; tokenId: string }> {
  if (input.sessionKind !== "web3") throw new Error("This wallet cannot pay its own gas on Ethereum");
  const progress = (stage: NameRegistrationStage | "approving") => {
    try {
      input.onProgress?.(stage);
    } catch {
      // Reporting progress must not abort a purchase.
    }
  };
  const chainId = input.chainId ?? landChainId();
  const network = landNetwork(chainId);
  const buyer = input.buyer.toLowerCase();
  if (!validateName(input.name).ok) throw new Error("Choose 2-15 letters or numbers for your NAME.");
  progress("preparing");
  await requireChain(input.provider, chainId);
  const reader = input.reader ?? walletReader(input.provider, chainId);
  const [price, token, registrar] = await Promise.all([
    readContract<bigint>(reader, network.nameController, controllerAbi, "PRICE", []),
    readContract<string>(reader, network.nameController, controllerAbi, "acceptedToken", []),
    readContract<string>(reader, network.nameController, controllerAbi, "registrar", []),
  ]);
  if (token.toLowerCase() !== network.mana || registrar.toLowerCase() !== network.nameRegistrar || price !== NAME_PRICE_WEI) {
    throw new NameControllerChangedError();
  }
  let hash = pendingNameRegistration(buyer, input.name);
  if (!hash) {
    if (!(await readContract<boolean>(reader, network.nameRegistrar, registrarAbi, "available", [input.name]))) throw new NameTakenError();
    const balance = await readContract<bigint>(reader, network.mana, erc20Abi, "balanceOf", [buyer]);
    if (balance < price) throw new InsufficientManaError(balance, price);
    progress("awaiting-confirmation");
    await ensureAllowance(input.provider, {
      chainId,
      token: network.mana,
      owner: buyer,
      spender: network.nameController,
      required: price,
      reader,
      wait: input.wait,
      onStage: (s) => (s === "approving" ? progress("approving") : undefined),
    });
    progress("awaiting-confirmation");
    hash = await sendTransaction(input.provider, {
      chainId,
      from: buyer,
      to: network.nameController,
      data: encodeCall(controllerAbi, "register", [input.name, buyer]),
    });
    savePending(buyer, input.name, hash);
  }
  progress("confirming");
  try {
    await waitForReceipt(input.provider, hash, input.wait);
  } catch (e) {
    if (!(e instanceof TxPendingError)) savePending(buyer, input.name, null);
    throw e;
  }
  const tokenId = nameTokenIdOf(input.name);
  const owner = await readContract<string>(reader, network.nameRegistrar, registrarAbi, "ownerOf", [BigInt(tokenId)]);
  if (owner.toLowerCase() !== buyer) throw new Error("The transaction confirmed, but NAME ownership could not be verified. Retry to check it again.");
  savePending(buyer, input.name, null);
  return { status: "registered", originTxHash: hash, tokenId };
}

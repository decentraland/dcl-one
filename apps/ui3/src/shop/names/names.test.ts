import { describe, expect, it, vi } from "vitest";

import { erc20Abi } from "../land/evm";
import { fakeWallet, pollCountedWait, stubClient } from "../land/testkit";
import { CHAIN } from "../model/chain";
import { manaUsdRate, usdCents } from "../model/money";
import { parseNameLabel } from "../model/asset";
import { presentPayment } from "../payment/present";
import { RATE, web2, web3, SIGNED_OUT, BOOK } from "../payment/fixtures";
import { LAND_NETWORKS } from "../land/contracts";
import {
  buildNameUseCreditsArgs,
  checkNameAvailability,
  controllerAbi,
  creditsChainFor,
  fetchNameCreditRoute,
  idToSalt,
  NameCreditsClosedError,
  NameNotRegisteredError,
  NameRouteCostTooHighError,
  nameTokenIdOf,
  NameTakenError,
  pollAcrossNameStatus,
  registerNameWithEthereumMana,
  registerNameWithUsdCredits,
  registrarAbi,
  sanitizeNameInput,
  sizeNameUsdCents,
  useCreditsAbi,
  validateName,
  InsufficientManaError,
  NAME_PRICE_WEI,
} from "./names";
import { interimNameOffer, NameAlreadyRegisteredError, serverNameOffer } from "./offer";
import { encodePurchaseOffer } from "../model/wire";
import { nameRegistration } from "../payment/fixtures";

const MAINNET = LAND_NETWORKS[CHAIN.ETHEREUM]!;
const BUYER = "0x0000000000000000000000000000000000000b0b";

describe("NAME input", () => {
  it("validates 2-15 ASCII letters or digits, reporting the most specific problem", () => {
    expect(validateName("Nacho")).toEqual({ ok: true });
    expect(validateName("")).toEqual({ ok: false, reason: "empty" });
    expect(validateName("a")).toEqual({ ok: false, reason: "too-short" });
    expect(validateName("a".repeat(16))).toEqual({ ok: false, reason: "too-long" });
    expect(validateName("ná")).toEqual({ ok: false, reason: "invalid-chars" });
    expect(validateName("two words")).toEqual({ ok: false, reason: "has-spaces" });
  });

  it("sanitizes keystrokes to what can be registered", () => {
    expect(sanitizeNameInput("Na ch-o!😀1234567890123")).toBe("Nacho1234567890");
  });

  it("derives the registrar token id as keccak256(lower(label)) (tests/fixtures/shop/chain-reads.json)", () => {
    expect(BigInt(nameTokenIdOf("Nacho")).toString(16)).toBe("2d3e1ff5ca46ab8fa2142019c7bd004b3a220beed1ab6c87214975beec275ceb");
  });

  it("sizes a NAME at 100 MANA of USD, rounded up", () => {
    expect(sizeNameUsdCents(RATE)).toBe(5000);
    expect(sizeNameUsdCents(manaUsdRate(33_333_333n, 8)!)).toBe(3334);
  });
});

describe("availability and the credit route", () => {
  it("asks the configured network's registrar", async () => {
    const wallet = fakeWallet({ account: BUYER, chainId: 1, reads: [{ abi: registrarAbi, name: "available", answer: ([name]) => name !== "taken" }] });
    const reader = { chainId: 1, call: (to: string, data: `0x${string}`) => wallet.request({ method: "eth_call", params: [{ to, data }] }) as Promise<`0x${string}`> };
    expect(await checkNameAvailability("free", { reader, chainId: 1 })).toBe("available");
    expect(await checkNameAvailability("taken", { reader, chainId: 1 })).toBe("taken");
  });

  it("fetches the signed route from /shop-api/v1/names/credit-route and names the two refusals", async () => {
    const route = { externalCall: { target: "0x1", selector: "0x12345678", data: "0x", expiresAt: 1, salt: "0x01" }, customExternalCallSignature: "0x", quoteId: "q", estimatedRouteDuration: 60, fromChainId: "137", toChainId: "1" };
    const ok = stubClient({ "GET /shop-api/v1/names/credit-route": { body: route } });
    expect(await fetchNameCreditRoute("Nacho", { client: ok })).toEqual(route);
    const q = new URL(ok.requests[0]!.url).searchParams;
    expect([q.get("name"), q.get("chainId"), q.get("provider")]).toEqual(["Nacho", "137", "across"]);
    expect(ok.requests[0]!.signed).toBe(true);
    const cost = stubClient({ "GET /shop-api/v1/names/credit-route": { status: 503, body: { error: "x", code: "ROUTE_COST_TOO_HIGH" } } });
    await expect(fetchNameCreditRoute("Nacho", { client: cost })).rejects.toBeInstanceOf(NameRouteCostTooHighError);
    const closed = stubClient({ "GET /shop-api/v1/names/credit-route": { status: 404, body: { error: "Name registration is not enabled", code: "NAMES_DISABLED" } } });
    await expect(fetchNameCreditRoute("Nacho", { client: closed })).rejects.toBeInstanceOf(NameCreditsClosedError);
  });

  it("builds the useCredits call: the credit pays, the route is the operation, 100 MANA credited", () => {
    const credit = { id: "0xab", amount: "102000000000000000000", availableAmount: "102000000000000000000", expiresAt: 1700000000, signature: "0xsig", contract: "0xcm" };
    const route = { externalCall: { target: "0xt", selector: "0x11223344", data: "0xdd", expiresAt: 5, salt: "0x02" }, customExternalCallSignature: "0xc5", quoteId: "q", estimatedRouteDuration: 1, fromChainId: "137", toChainId: "1" };
    const args = buildNameUseCreditsArgs(credit, route);
    expect(args).toMatchObject({ maxCreditedValue: "100000000000000000000", maxUncreditedValue: "0", creditsSignatures: ["0xsig"] });
    expect(args.credits[0]!.salt).toBe(idToSalt("0xab"));
    expect(buildNameUseCreditsArgs({ ...credit, availableAmount: "60000000000000000000" }, route).maxUncreditedValue).toBe("40000000000000000000");
    expect(useCreditsAbi[0].name).toBe("useCredits");
    expect(creditsChainFor(CHAIN.SEPOLIA)).toBe(CHAIN.AMOY);
  });
});

describe("Across fill", () => {
  const answer = (body: unknown) => async () => new Response(JSON.stringify(body));
  it("keeps 'filled' with an unreported outcome apart from success", async () => {
    expect(await pollAcrossNameStatus("0xo", { fetchImpl: answer({ status: "filled", fillTx: "0xd" }), maxAttempts: 1 })).toEqual({ status: "filled", destinationTxHash: "0xd", actionsSucceeded: null });
    expect(await pollAcrossNameStatus("0xo", { fetchImpl: answer({ status: "refunded" }), maxAttempts: 1 })).toEqual({ status: "refunded", destinationTxHash: null, actionsSucceeded: false });
    expect(await pollAcrossNameStatus("0xo", { fetchImpl: answer({ status: "pending" }), maxAttempts: 2, intervalMs: 0 })).toMatchObject({ status: "pending" });
  });
});

const ROUTE = { externalCall: { target: "0x000000000000000000000000000000000000e0e0", selector: "0x11223344", data: "0xdd", expiresAt: 5, salt: "0x02" }, customExternalCallSignature: "0xc5", quoteId: "q", estimatedRouteDuration: 1, fromChainId: "137", toChainId: "1", provider: "across" };
const CREDIT = { id: `0x${"ab".repeat(32)}`, amount: "102000000000000000000", availableAmount: "102000000000000000000", expiresAt: 1700000000, signature: "0x5151", contract: "0x000000000000000000000000000000000000c0de" };

function creditsServer(opts: { authorize?: { status?: number; body: unknown } } = {}) {
  return stubClient({
    "GET /shop-api/v1/names/credit-route": { body: ROUTE },
    "POST /shop-api/v1/credit-authorizations": opts.authorize ?? { body: { credit: CREDIT, maxCreditedValue: "102000000000000000000", usdCents: 5000, oracleRate: "1" } },
    "POST /shop-api/v1/credit-authorizations/cancel": { body: { released: 1, retired: 0, spendableAtSeconds: null } },
    "POST /shop-api/v1/credit-authorizations/submitted": { body: { recorded: 1 } },
  });
}

describe("CLAIM_NAME_WITH_CREDITS: register with credits", () => {
  const fill = (body: unknown) => ({ fetchImpl: async () => new Response(JSON.stringify(body)), maxAttempts: 1, intervalMs: 0 });

  it("authorizes the quote for the NAME, submits useCredits to the credit's manager, reports it and follows the fill", async () => {
    const wallet = fakeWallet({ account: BUYER, chainId: CHAIN.POLYGON, writes: [useCreditsAbi] });
    const client = creditsServer();
    const stages: string[] = [];
    const out = await registerNameWithUsdCredits({
      name: "Nacho",
      buyer: BUYER,
      creditsCents: usdCents(5000),
      quotedCents: usdCents(5000),
      provider: wallet,
      client,
      acrossPoll: fill({ status: "filled", fillTx: "0xfill", actionsSucceeded: true }),
      wait: pollCountedWait(20),
      onProgress: (s) => stages.push(s),
    });
    expect(out).toEqual({ status: "registered", originTxHash: wallet.sent[0] && expect.any(String), destinationTxHash: "0xfill" });
    expect(stages).toEqual(["preparing", "awaiting-confirmation", "confirming", "registering"]);
    const authorize = client.requests.find((r) => r.url.endsWith("/credit-authorizations"))!;
    expect(JSON.parse(authorize.body!)).toEqual({ usdPriceCents: 5000, name: "Nacho", source: "website" });
    expect(wallet.sent[0]).toMatchObject({ to: CREDIT.contract, fn: "useCredits" });
    await vi.waitFor(() => expect(client.requests.some((r) => r.url.endsWith("/credit-authorizations/submitted"))).toBe(true), { timeout: 10_000 });
    expect(client.requests.some((r) => r.url.endsWith("/credit-authorizations/cancel"))).toBe(false);
  });

  it("releases the reservation when the wallet refuses before anything was sent", async () => {
    const wallet = fakeWallet({ account: BUYER, chainId: CHAIN.POLYGON, writes: [useCreditsAbi] });
    wallet.reject.add("eth_sendTransaction");
    const client = creditsServer();
    await expect(registerNameWithUsdCredits({ name: "Nacho", buyer: BUYER, creditsCents: usdCents(5000), quotedCents: usdCents(5000), provider: wallet, client })).rejects.toMatchObject({ code: 4001 });
    expect(client.requests.some((r) => r.url.endsWith("/credit-authorizations/cancel"))).toBe(true);
  });

  it("keeps the reservation and reports pending when the origin transaction has not confirmed", async () => {
    const wallet = fakeWallet({ account: BUYER, chainId: CHAIN.POLYGON, writes: [useCreditsAbi], autoMine: false });
    const client = creditsServer();
    const out = await registerNameWithUsdCredits({ name: "Nacho", buyer: BUYER, creditsCents: usdCents(5000), quotedCents: usdCents(5000), provider: wallet, client, wait: pollCountedWait(10) });
    expect(out.status).toBe("pending");
    expect(client.requests.some((r) => r.url.endsWith("/credit-authorizations/cancel"))).toBe(false);
  });

  it("never releases a consumed credit when the bridge refunded: NameNotRegisteredError", async () => {
    const wallet = fakeWallet({ account: BUYER, chainId: CHAIN.POLYGON, writes: [useCreditsAbi] });
    const client = creditsServer();
    await expect(
      registerNameWithUsdCredits({ name: "Nacho", buyer: BUYER, creditsCents: usdCents(5000), quotedCents: usdCents(5000), provider: wallet, client, acrossPoll: fill({ status: "refunded" }), wait: pollCountedWait(20) }),
    ).rejects.toBeInstanceOf(NameNotRegisteredError);
    expect(client.requests.some((r) => r.url.endsWith("/credit-authorizations/cancel"))).toBe(false);
  });

  it("stops before submitting when the credit route is closed, and never reserves", async () => {
    const wallet = fakeWallet({ account: BUYER, chainId: CHAIN.POLYGON });
    const client = stubClient({ "GET /shop-api/v1/names/credit-route": { status: 404, body: { code: "NAMES_DISABLED", error: "Name registration is not enabled" } } });
    await expect(registerNameWithUsdCredits({ name: "Nacho", buyer: BUYER, creditsCents: usdCents(5000), quotedCents: usdCents(5000), provider: wallet, client })).rejects.toBeInstanceOf(NameCreditsClosedError);
    expect(client.requests.map((r) => new URL(r.url).pathname)).toEqual(["/shop-api/v1/names/credit-route"]);
    expect(wallet.sent).toEqual([]);
  });

  it("on the mixed rail approves only the MANA gap and refuses a gap above what was agreed", async () => {
    const wallet = fakeWallet({ account: BUYER, chainId: CHAIN.POLYGON, reads: [{ abi: erc20Abi, name: "allowance", answer: () => 0n }], writes: [useCreditsAbi] });
    const mixedCredit = { ...CREDIT, amount: "60000000000000000000", availableAmount: "60000000000000000000" };
    const client = creditsServer({ authorize: { body: { credit: mixedCredit, maxCreditedValue: "60000000000000000000", usdCents: 3000, oracleRate: "1" } } });
    await expect(
      registerNameWithUsdCredits({ name: "Nacho", buyer: BUYER, creditsCents: usdCents(3000), quotedCents: usdCents(5000), maxManaWei: 39n * 10n ** 18n, provider: wallet, client }),
    ).rejects.toThrow("moved past what was agreed");
    expect(wallet.sent).toEqual([]);
    const ok = await registerNameWithUsdCredits({
      name: "Nacho",
      buyer: BUYER,
      creditsCents: usdCents(3000),
      quotedCents: usdCents(5000),
      maxManaWei: 40n * 10n ** 18n,
      provider: wallet,
      client: creditsServer({ authorize: { body: { credit: mixedCredit, maxCreditedValue: "60000000000000000000", usdCents: 3000, oracleRate: "1" } } }),
      acrossPoll: fill({ status: "filled", actionsSucceeded: true }),
      wait: pollCountedWait(20),
    });
    expect(ok.status).toBe("registered");
    expect(wallet.sent.map((t) => t.fn)).toEqual(["approve", "useCredits"]);
    expect(wallet.sent[0]!.args).toEqual([CREDIT.contract, 40n * 10n ** 18n]);
  });
});

describe("CLAIM_NAME: register with MANA on Ethereum", () => {
  const controllerReads = (over: { price?: bigint; token?: string; available?: boolean; balance?: bigint; allowance?: bigint; owner?: string } = {}) => [
    { abi: controllerAbi, name: "PRICE", answer: () => over.price ?? NAME_PRICE_WEI },
    { abi: controllerAbi, name: "acceptedToken", answer: () => over.token ?? MAINNET.mana },
    { abi: controllerAbi, name: "registrar", answer: () => MAINNET.nameRegistrar },
    { abi: registrarAbi, name: "available", answer: () => over.available ?? true },
    { abi: registrarAbi, name: "ownerOf", answer: () => over.owner ?? BUYER },
    { abi: erc20Abi, name: "balanceOf", answer: () => over.balance ?? 500n * 10n ** 18n },
    { abi: erc20Abi, name: "allowance", answer: () => over.allowance ?? 0n },
  ];

  it("approves 100 MANA to the controller, registers to the buyer and verifies ownership", async () => {
    sessionStorage.clear();
    const wallet = fakeWallet({ account: BUYER, chainId: 1, reads: controllerReads(), writes: [controllerAbi] });
    const out = await registerNameWithEthereumMana({ name: "Nacho", buyer: BUYER, provider: wallet, sessionKind: "web3", chainId: 1, wait: pollCountedWait(20) });
    expect(out.tokenId).toBe(nameTokenIdOf("Nacho"));
    expect(wallet.sent.map((t) => [t.to, t.fn])).toEqual([
      [MAINNET.mana, "approve"],
      [MAINNET.nameController, "register"],
    ]);
    expect(wallet.sent[1]!.args).toEqual(["Nacho", BUYER]);
  });

  it("refuses a wallet session that cannot pay Ethereum gas, a taken NAME, a short balance and a changed controller", async () => {
    const wallet = fakeWallet({ account: BUYER, chainId: 1, reads: controllerReads() });
    await expect(registerNameWithEthereumMana({ name: "Nacho", buyer: BUYER, provider: wallet, sessionKind: "web2", chainId: 1 })).rejects.toThrow("cannot pay its own gas");
    sessionStorage.clear();
    await expect(registerNameWithEthereumMana({ name: "Nacho", buyer: BUYER, provider: fakeWallet({ account: BUYER, chainId: 1, reads: controllerReads({ available: false }) }), sessionKind: "web3", chainId: 1 })).rejects.toBeInstanceOf(NameTakenError);
    await expect(registerNameWithEthereumMana({ name: "Nacho", buyer: BUYER, provider: fakeWallet({ account: BUYER, chainId: 1, reads: controllerReads({ balance: 99n }) }), sessionKind: "web3", chainId: 1 })).rejects.toBeInstanceOf(InsufficientManaError);
    await expect(registerNameWithEthereumMana({ name: "Nacho", buyer: BUYER, provider: fakeWallet({ account: BUYER, chainId: 1, reads: controllerReads({ price: 1n }) }), sessionKind: "web3", chainId: 1 })).rejects.toThrow("configuration has changed");
  });

  it("follows the same registration on a retry instead of paying twice", async () => {
    sessionStorage.clear();
    const wallet = fakeWallet({ account: BUYER, chainId: 1, reads: controllerReads({ allowance: NAME_PRICE_WEI }), writes: [controllerAbi], autoMine: false });
    await expect(registerNameWithEthereumMana({ name: "Nacho", buyer: BUYER, provider: wallet, sessionKind: "web3", chainId: 1, wait: pollCountedWait(5) })).rejects.toMatchObject({ name: "TxPendingError" });
    const hash = [...wallet.receipts.keys()][0]!;
    wallet.receipts.set(hash, 1);
    const out = await registerNameWithEthereumMana({ name: "Nacho", buyer: BUYER, provider: wallet, sessionKind: "web3", chainId: 1, wait: pollCountedWait(5) });
    expect(out.originTxHash).toBe(hash);
    expect(wallet.sent.filter((t) => t.fn === "register")).toHaveLength(1);
  });
});

describe("the NAME registration offer", () => {
  const label = parseNameLabel("Nacho")!;
  const available = (free: boolean) => {
    const wallet = fakeWallet({ account: BUYER, chainId: 1, reads: [{ abi: registrarAbi, name: "available", answer: () => free }] });
    return { chainId: 1, call: (to: string, data: `0x${string}`) => wallet.request({ method: "eth_call", params: [{ to, data }] }) as Promise<`0x${string}`> };
  };
  const quoting = (credits: string | null) => stubClient({ "POST /shop-api/v1/credit-quotes": { body: { items: [], amounts: [credits] } } });

  it("presents credits first and default, MANA (the registrar, on Ethereum) second for a web3 wallet", async () => {
    const offer = await interimNameOffer({ client: quoting("500"), reader: available(true), chainId: 1 })(label, web3(), new AbortController().signal);
    const view = presentPayment(offer, web3(), { report: () => {} });
    expect(view.inconsistencies).toEqual([]);
    expect(view.primary).toBe("credits");
    expect(view.options.map((o) => o.method)).toEqual(["credits", "mana"]);
    expect(view.options[0]!.quote).toEqual({ method: "credits", quoted: 5000 });
    expect(view.options[1]!.quote).toEqual({ method: "mana", amount: NAME_PRICE_WEI });
    expect(offer.settlement.find((s) => s.method === "mana")?.key).toMatchObject({ chainId: 1, contract: MAINNET.nameController });
  });

  it("offers a web2 or signed-out buyer credits only, and never MANA", async () => {
    for (const session of [web2(), SIGNED_OUT]) {
      const offer = await interimNameOffer({ client: quoting("500"), reader: available(true), chainId: 1 })(label, session, new AbortController().signal);
      expect(presentPayment(offer, session, { report: () => {} }).options.map((o) => o.method)).toEqual(["credits"]);
    }
  });

  it("keeps credits as the main action with an explanation when the credits price is unavailable", async () => {
    const offer = await interimNameOffer({ client: quoting(null), reader: available(true), chainId: 1 })(label, web3(), new AbortController().signal);
    const view = presentPayment(offer, web3(), { report: () => {} });
    expect(view.inconsistencies).toEqual([]);
    expect(view.primary).toBe("credits");
    expect(view.options[0]!.state).toEqual({ kind: "unavailable", missing: "manaUsdRate" });
    expect(view.options[1]!.state.kind).toBe("ready");
  });

  it("refuses to present a registration for a NAME that exists", async () => {
    await expect(interimNameOffer({ client: quoting("500"), reader: available(false), chainId: 1 })(label, web3(), new AbortController().signal)).rejects.toBeInstanceOf(NameAlreadyRegisteredError);
  });

  it("decodes the server's offer when the quote route answers", async () => {
    const wire = encodePurchaseOffer(nameRegistration(web3(BOOK.buyer)));
    const offer = await serverNameOffer(async () => wire)(label, web3(), new AbortController().signal);
    expect(offer.sale).toBe("primaryNameRegistration");
  });
});

describe("the offer seam", () => {
  it("asks the host's offer source for the registrar token and the label, and reads null as registered", async () => {
    const { seamNameOffer } = await import("./offer");
    const { missingOfferRoute } = await import("../detail/offers");
    expect(seamNameOffer(missingOfferRoute)).toBeNull();
    const label = parseNameLabel("Nacho")!;
    const asked: unknown[] = [];
    const source = seamNameOffer(async (queries) => {
      asked.push(...queries);
      return { [queries[0]!.key]: nameRegistration(web3()) };
    }, 1)!;
    const offer = await source(label, web3(), new AbortController().signal);
    expect(offer.sale).toBe("primaryNameRegistration");
    expect(asked[0]).toMatchObject({ listing: { name: "Nacho" }, asset: { chainId: 1, contract: MAINNET.nameRegistrar, token: { kind: "token", tokenId: nameTokenIdOf("Nacho") } } });
    const taken = seamNameOffer(async (queries) => ({ [queries[0]!.key]: null }), 1)!;
    await expect(taken(label, web3(), new AbortController().signal)).rejects.toBeInstanceOf(NameAlreadyRegisteredError);
  });
});

// The credits+MANA rail inside the web3 MANA option, run end to end on the checkout demo's fake node,
// chain and relay: the manager's MANA allowance approved first (exactly the MANA shown), ONE credit
// authorized at the whole price for the credits leg, the MANA gap checked against what was shown, and a
// declined signature releasing the credit.

import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { decodeFunctionData, type Hex } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setSplitPaymentForTests } from "@shop/payment/present";

import { CREDITS_MANAGER_ABI, OFFCHAIN_META_TX_ABI } from "@shop/checkout/chain/abi";

import { requireContracts } from "../../checkout/chain/contracts";
import { batchAuthorization, createFakeLocks, creatorTrade, relayedApproval, splitSaleTransaction, type FakeHandlers } from "../../checkout/fakes";
import { encodeExecuteMetaTransaction } from "../../checkout/chain/metatx";
import { encodeAuthorizationCall, exactGrantLock, getManaSpendingAuthorization, metaTxTargetFor } from "../../checkout/lib/authorizations";
import { CHAIN } from "@shop/model/chain";
import { createDemo, DemoShell } from "../../checkout/demo";
import { usdCents } from "@shop/model/money";
import { presentPayment, type MixedPayment } from "@shop/payment/present";
import { BOOK, creatorOrder, storeMint } from "@shop/payment/fixtures";
import type { ManaWei } from "@shop/model/money";
import type { PurchaseOffer } from "@shop/model/purchase";
import { ASSET_TYPE } from "../../checkout/purchase/encoding";
import { SHOP_QUERY_KEYS } from "../../checkout/lib/after-purchase";
import { readSplitHold, readSplitHoldRecord, SPLIT_EXPIRY_CONFIRM_MS, SPLIT_HOLD_MS, splitHoldLine, type SplitHold } from "../../checkout/purchase/splitHold";
import { splitEnvelope, type SplitSale } from "../../checkout/purchase/splitEvidence";
import CombinedBuy from "./CombinedBuy";

afterEach(() => {
  sessionStorage.clear();
  localStorage.clear();
});

/** The page's query client, as the dialog's own page holds it. */
let pageQueries: QueryClient | null = null;
function QueryProbe() {
  pageQueries = useQueryClient();
  return null;
}

const MANA = 10n ** 18n;

/** The useCredits arguments a relayed CreditsManager meta-transaction carries. */
function relayedUseCredits(body: unknown): { maxCreditedValue: bigint; maxUncreditedValue: bigint } {
  const data = (body as { transactionData: { params: [string, Hex] } }).transactionData.params[1];
  const meta = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data });
  return decodeFunctionData({ abi: CREDITS_MANAGER_ABI, data: meta.args[1] as Hex }).args[0] as { maxCreditedValue: bigint; maxUncreditedValue: bigint };
}

/**
 * The node answers a credit for the Credits part (`creditsCents`), worth `value` (1 MANA by default), spendable `expiresIn`
 * s; `handlers` answer the node's other routes.
 */
function setup(signedFor?: string, confirm?: "none" | "reverted", value = MANA, expiresIn?: number, handlers: FakeHandlers = {}) {
  const demo = createDemo({
    session: "web3",
    ...(confirm ? { confirm } : {}),
    handlers: {
      authorizeCredit: ({ body }) => {
        const cents = (body as { creditsCents: number }).creditsCents;
        const one = batchAuthorization([{ usdPriceCents: cents }]);
        const credit = {
          ...one.credit,
          amount: value.toString(),
          availableAmount: value.toString(),
          ...(signedFor ? { contract: signedFor } : {}),
          ...(expiresIn ? { expiresAt: Math.floor(Date.now() / 1000) + expiresIn } : {}),
        };
        return { ...one, credit, maxCreditedValue: value.toString(), usdCents: cents };
      },
      ...handlers,
    },
  });
  const offer = storeMint(demo.session);
  const view = presentPayment(offer, demo.session, { creditsBalance: usdCents(50), report: () => {} });
  const mixed = view.options.find((o) => o.method === "mana")!.mixed as MixedPayment;
  const line = { asset: offer.asset, listing: { acquisition: "store" as const }, display: { name: "Rainbow Hat" } };
  const shell = () => (
    <DemoShell demo={demo} path="/shop/item/x/7">
      <QueryProbe />
      <CombinedBuy line={line} offer={offer} mixed={mixed} onClose={() => {}} portal={false} />
    </DemoShell>
  );
  const shown = render(shell());
  return { demo, mixed, line, unmount: shown.unmount, again: () => render(shell()) };
}


// The split (Credits and MANA together) follows its rollout (`dapps-shop-credits-mana-split`), off unless a render says
// on: these tests cover it as it runs once it is offered.
let undoSplit: () => void = () => {};
beforeEach(() => {
  undoSplit = setSplitPaymentForTests(true);
});
afterEach(() => undoSplit());

describe("buying with Credits and MANA", () => {
  it("approves the Credits manager exactly the MANA shown, authorizes one credit at the whole price for the credits leg, and relays one useCredits", async () => {
    const { demo, mixed } = setup();
    expect(mixed).toMatchObject({ priceCents: 115, credits: 5, creditsCents: 50 });
    expect(screen.getByTestId("combined-credits")).toHaveTextContent("5 Credits");
    expect(screen.getByTestId("combined-mana")).toHaveTextContent("1.3 MANA");
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(demo.shop.log.some((r) => r.name === "authorizeCredit")).toBe(false);
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    const auth = demo.shop.log.filter((r) => r.name === "authorizeCredit");
    expect(auth).toHaveLength(1);
    expect(auth[0]!.body).toMatchObject({ usdPriceCents: 115, creditsCents: 50, contractAddress: BOOK.collection, itemId: "7" });
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    expect(demo.relay.posts).toHaveLength(2);
    expect(relayedApproval(demo.relay.posts[0])).toEqual({ owner: BOOK.buyer.toLowerCase(), spender: manager, amount: mixed.manaWei });
    // The 2.3 MANA mint: the 1 MANA credit and the 1.3 MANA gap, each its own cap.
    expect(relayedUseCredits(demo.relay.posts[1])).toMatchObject({ maxCreditedValue: MANA, maxUncreditedValue: (13n * MANA) / 10n });
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
  });

  it("a finished purchase has the page read its MANA balance again", async () => {
    setup();
    const balance = [SHOP_QUERY_KEYS.manaBalance, BOOK.buyer.toLowerCase(), 137];
    act(() => void pageQueries!.setQueryData(balance, 10n ** 21n));
    expect(pageQueries!.getQueryState(balance)?.isInvalidated).toBe(false);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(pageQueries!.getQueryState(balance)?.isInvalidated).toBe(true);
  });

  it("a credit below the node's share (which would leave more MANA than shown) is refused, nothing is relayed, and it is released", async () => {
    // A 0.5 MANA credit on the 2.3 MANA mint, where the share is 1 MANA: 1.8 MANA left, above the 1.3 confirmed.
    const { demo } = setup(undefined, undefined, MANA / 2n);
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toHaveTextContent("Nothing was charged.");
    expect(demo.relay.posts).toHaveLength(0);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
  });

  it("a listing paid in MANA: the credit is the share of its MANA, the gap the rest, each its own cap (the split vector, 91 wei)", async () => {
    // Quoted 30 cents and 91 MANA wei (the vector's 101 wei after its 10% coupon); 10 cents of Credits held.
    const demo = createDemo({
      session: "web3",
      resolvers: {
        trade: async () => creatorTrade({ received: [{ assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: "91", extra: "0x", beneficiary: BOOK.creator }] }),
      },
      handlers: {
        authorizeCredit: ({ body }) => {
          const { usdPriceCents, creditsCents } = body as { usdPriceCents: number; creditsCents: number };
          const value = ((91n * BigInt(creditsCents)) / BigInt(usdPriceCents)).toString();
          const one = batchAuthorization([{ usdPriceCents: creditsCents }]);
          return { ...one, credit: { ...one.credit, amount: value, availableAmount: value }, maxCreditedValue: value, usdCents: creditsCents };
        },
      },
    });
    const listed = creatorOrder(demo.session);
    const offer: PurchaseOffer = {
      ...listed,
      quotes: [
        { method: "credits", quoted: usdCents(30) },
        { method: "mana", amount: 91n as ManaWei },
      ],
      splittable: true,
    };
    const mixed = presentPayment(offer, demo.session, { creditsBalance: usdCents(10), report: () => {} }).options.find((o) => o.method === "mana")!.mixed!;
    expect(mixed).toEqual({ priceCents: 30, creditsCents: 10, credits: 1, manaWei: 61n });
    const line = { asset: offer.asset, listing: { tradeId: "trade-creator-1" }, display: { name: "Rainbow Hat" } };
    render(
      <DemoShell demo={demo} path="/shop/item/x/7">
        <CombinedBuy line={line} offer={offer} mixed={mixed} onClose={() => {}} portal={false} />
      </DemoShell>,
    );
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    const auth = demo.shop.log.filter((r) => r.name === "authorizeCredit");
    expect(auth.map((r) => r.body)).toEqual([expect.objectContaining({ usdPriceCents: 30, creditsCents: 10, tradeId: "trade-creator-1" })]);
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    expect(relayedApproval(demo.relay.posts[0])).toEqual({ owner: BOOK.buyer.toLowerCase(), spender: manager, amount: 61n });
    expect(relayedUseCredits(demo.relay.posts[1])).toMatchObject({ maxCreditedValue: 30n, maxUncreditedValue: 61n });
  });

  it("a USD-pegged listing is refused before any credit is authorized: its MANA is only fixed at settlement", async () => {
    let resolved = 0;
    const demo = createDemo({
      session: "web3",
      resolvers: {
        trade: async () => {
          resolved++;
          return creatorTrade();
        },
      },
    });
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    const offer = creatorOrder(demo.session);
    // Never offered for such a line (present.ts); the dialog refuses it all the same.
    const mixed: MixedPayment = { priceCents: usdCents(100), creditsCents: usdCents(50), credits: 5, manaWei: (MANA / 2n) as MixedPayment["manaWei"] };
    const line = { asset: offer.asset, listing: { tradeId: "trade-creator-1" }, display: { name: "Rainbow Hat" } };
    render(
      <DemoShell demo={demo} path="/shop/item/x/7">
        <CombinedBuy line={line} offer={offer} mixed={mixed} onClose={() => {}} portal={false} />
      </DemoShell>,
    );
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toHaveTextContent("Nothing was charged.");
    expect(resolved).toBeGreaterThan(0);
    expect(demo.shop.log.some((r) => r.name === "authorizeCredit")).toBe(false);
    expect(demo.relay.posts).toHaveLength(0);
  });

  it("a credit above the node's share (here the whole price, no MANA left) is refused, never paid with the gap shown before, and released", async () => {
    // A 2.3 MANA credit on the 2.3 MANA mint, where the share is 1 MANA: the terms the buyer confirmed no longer hold.
    const { demo } = setup(undefined, undefined, (23n * MANA) / 10n);
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toHaveTextContent("We couldn't complete the purchase. Nothing was charged.");
    expect(demo.relay.posts).toHaveLength(0);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
  });

  it("a declined signature releases the credit and says nothing was charged", async () => {
    const { demo } = setup();
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    demo.wallet.next = "reject";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText("You cancelled the request in your wallet. Nothing was charged.", {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
  });

  it("refuses a credit signed for a manager the Shop does not know, before approving or relaying anything", async () => {
    const stranger = "0x000000000000000000000000000000000000dead";
    const { demo } = setup(stranger);
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    const before = new Map(demo.chain.allowances);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(0);
    expect(demo.chain.allowances).toEqual(before);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
  });

  it("a split left confirming is recorded for this account and item, so a dialog opened again offers no second one", async () => {
    const { line, unmount, again } = setup(undefined, "none");
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 8000 })).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).not.toBeNull();
    unmount();
    again();
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    expect(screen.getByTestId("combined-check-again")).toBeEnabled();
  });
});

describe("a split whose outcome is not known", () => {
  const managerKey = () => `${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase()}`;
  /** The buyer's meta-transaction nonce on MANA, as the fake chain answers `getNonce`. */
  const manaNonceKey = () => `${requireContracts(CHAIN.POLYGON).mana.address.toLowerCase()}:${BOOK.buyer.toLowerCase()}`;
  const authorizations = (demo: ReturnType<typeof createDemo>) => demo.shop.log.filter((r) => r.name === "authorizeCredit").length;

  it("a sale the relay may have sent (its answer lost) is held confirming, kept on reopen, and nothing is authorized or sent again", async () => {
    const { demo, line, unmount, again } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    demo.relay.state.mode = "fail-after-send";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    expect(authorizations(demo)).toBe(1);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
    const held = readSplitHold(BOOK.buyer, splitHoldLine(line.asset));
    expect(held).toMatchObject({ txHash: null, chainId: CHAIN.POLYGON, creditId: expect.any(String), creditExpiresAt: expect.any(Number) });
    unmount();
    again();
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    expect(screen.getByTestId("combined-check-again")).toBeDisabled();
    expect(demo.relay.posts).toHaveLength(1);
    expect(authorizations(demo)).toBe(1);
  });

  it("control: a sale the relay marked as not sent is not held; declined in the wallet, its credit is released", async () => {
    const { demo, line } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    demo.relay.state.mode = "refuse";
    demo.wallet.next = "reject";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText("You cancelled the request in your wallet. Nothing was charged.", {}, { timeout: 5000 })).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toBeNull();
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
    expect(screen.getByTestId("combined-confirm")).toBeEnabled();
  });

  it.each([
    ["refuses", true],
    ["keeps without a word", false],
  ] as const)("a sale that provably sent nothing, whose record the storage %s, says what happened and stays held: Check again shows that once the record can go", async (_, throws) => {
    const { demo, line } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    demo.relay.state.mode = "refuse";
    demo.wallet.next = "reject";
    const removeItem = Storage.prototype.removeItem;
    const stuck = vi.spyOn(Storage.prototype, "removeItem").mockImplementation(function (this: Storage, key: string) {
      if (!key.startsWith("dcl-shop-buynow-split:")) return removeItem.call(this, key);
      if (throws) throw new DOMException("The operation is insecure.", "SecurityError");
    });
    const declined = "You cancelled the request in your wallet. Nothing was charged.";
    try {
      await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
      expect(await screen.findByText(declined, {}, { timeout: 5000 })).toBeTruthy();
      expect(screen.getByTestId("combined-check-notice")).toHaveTextContent("This browser couldn't update its record of this purchase. Check again in a moment.");
      expect(screen.queryByText(/was sent and is still confirming/)).toBeNull();
      // Still held, as it would be on the next open: no Pay beside it.
      expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).not.toBeNull();
      expect(screen.queryByTestId("combined-confirm")).toBeNull();
    } finally {
      stuck.mockRestore();
    }
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(await screen.findByTestId("combined-confirm")).toBeEnabled();
    expect(screen.getByText(declined)).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toBeNull();
    // Its credit went back once, when the sale failed; nothing more on Check again.
    expect(demo.shop.log.filter((r) => r.name === "cancelCreditAuthorizations")).toHaveLength(1);
  });

  it("after a refusal the relay marked as not sent, a direct sale whose wallet lost its answer (4900) is held: its credit is kept", async () => {
    const { demo, line } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    demo.relay.state.mode = "refuse";
    demo.wallet.next = "lost";
    // What the record holds the moment the wallet is handed the sale.
    const seen: { record: SplitHold | null } = { record: null };
    const request = demo.wallet.request.bind(demo.wallet);
    demo.wallet.request = (async (args: Parameters<typeof request>[0]) => {
      if (args.method === "eth_sendTransaction") seen.record = readSplitHoldRecord(BOOK.buyer, splitHoldLine(line.asset));
      return request(args);
    }) as typeof demo.wallet.request;
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.wallet.sent).toHaveLength(1);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toMatchObject({ txHash: null, creditId: expect.any(String) });
    // The identity of the very call the wallet was handed, kept before it was: not the relay's refused one.
    expect(seen.record?.sale?.envelope).toBe(splitEnvelope(demo.wallet.sent[0]!.data));
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))?.sale?.envelope).toBe(splitEnvelope(demo.wallet.sent[0]!.data));
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
  });

  it("a direct sale the wallet sent and lost the answer to is settled by its credit's record: its own useCredits, read by its sender, completes it", async () => {
    // The credit the node issues (batchAuthorization's salt), listed SETTLED by the wallet's transaction once it says so.
    const credit = `0x${"a1".repeat(32)}`;
    const listed: { txHash: string | null } = { txHash: null };
    const { demo, line, unmount, again } = setup(undefined, undefined, MANA, 60, {
      userPurchases: () => ({
        purchases: listed.txHash ? [{ id: `${credit}:0`, status: "SETTLED", usdCents: 50, createdAt: 1, txHash: listed.txHash }] : [],
        total: listed.txHash ? 1 : 0,
      }),
    });
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    demo.relay.state.mode = "refuse";
    demo.wallet.next = "lost";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toMatchObject({ txHash: null, sale: expect.objectContaining({ creditId: credit }) });
    // The wallet's own transaction, mined: the node lists the credit SETTLED by it.
    const [sentHash] = [...demo.chain.transactions.entries()].find(([, tx]) => tx.from === BOOK.buyer.toLowerCase())!;
    demo.chain.receipts.set(sentHash, "success");
    listed.txHash = sentHash;
    // Past its hold and its credit's expiry, opened again: Check again has something to do.
    const clock = vi.spyOn(Date, "now").mockReturnValue(Date.now() + SPLIT_HOLD_MS + 120_000);
    try {
      unmount();
      again();
      await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
      expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    } finally {
      clock.mockRestore();
    }
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
  });

  it("the record written before the sale leaves carries its terms: a tab that dies mid-send leaves them for any surface to settle", async () => {
    const { demo, line, unmount, again } = setup();
    unmount();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    // The relay is slow to answer: the sale is out of this page's hands, its outcome not yet known.
    let answer!: () => void;
    const slow = new Promise<void>((resolve) => (answer = resolve));
    const relayed = demo.services.fetchImpl!;
    // The signed execution the relay is asked to send, and what the record holds the moment it is asked.
    const seen: { posted: Hex | null; record: SplitHold | null } = { posted: null, record: null };
    demo.services.fetchImpl = (async (...args: Parameters<typeof fetch>) => {
      if (!seen.posted) {
        seen.posted = (JSON.parse(String(args[1]?.body)) as { transactionData: { params: [string, Hex] } }).transactionData.params[1];
        seen.record = readSplitHoldRecord(BOOK.buyer, splitHoldLine(line.asset));
      }
      await slow;
      return relayed(...args);
    }) as typeof fetch;
    again();
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await waitFor(() => expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).not.toBeNull());
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toMatchObject({
      txHash: null,
      sale: {
        buyer: BOOK.buyer.toLowerCase(),
        manager: requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase(),
        creditWei: MANA.toString(),
        gapWei: ((13n * MANA) / 10n).toString(),
        target: BOOK.store.contract.toLowerCase(),
        hold: { line: splitHoldLine(line.asset) },
      },
    });
    // Its envelope, the identity of those very bytes, was kept before they went to the relay.
    expect(seen.record?.sale?.envelope).toBe(splitEnvelope(seen.posted!));
    answer();
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
  });

  const OTHER = `0x${"0e".repeat(32)}`;
  const holdKey = (line: ReturnType<typeof setup>["line"]) => `dcl-shop-buynow-split:v1:${BOOK.buyer.toLowerCase()}:${splitHoldLine(line.asset)}`;
  const nowSeconds = () => Math.floor(Date.now() / 1000);
  /** Check again clicked, and done reading (the button back on). */
  async function checkAgain() {
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
  }

  it("a sale sent sixteen minutes ago with a hash, its credit still spendable, is kept on reopen: no Pay, and a missing receipt lets nothing go", async () => {
    const { demo, line, unmount, again } = setup();
    unmount();
    const sale: SplitSale = {
      buyer: BOOK.buyer.toLowerCase(),
      chainId: CHAIN.POLYGON,
      manager: requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase(),
      creditId: OTHER,
      creditWei: MANA.toString(),
      gapWei: ((13n * MANA) / 10n).toString(),
      target: BOOK.store.contract.toLowerCase(),
      selector: "0x12345678",
      callData: "0xabcdef",
    };
    const record = { at: Date.now() - 16 * 60 * 1000, attempt: "a1", txHash: `0x${"5a".repeat(32)}`, chainId: CHAIN.POLYGON, creditId: OTHER, creditExpiresAt: nowSeconds() + 300, sale };
    localStorage.setItem(holdKey(line), JSON.stringify(record));
    again();
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    await checkAgain();
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toMatchObject({ attempt: "a1", txHash: record.txHash });
    // Nothing on chain to read: the node is asked, lists nothing for its credit, and that settles nothing.
    expect(demo.shop.log.some((r) => r.name === "userPurchases")).toBe(true);
  });

  it("a dialog left open past the hold keeps a sale with a hash whose receipt is not in, while its credit can still be spent", async () => {
    const { demo, line } = setup(undefined, "none", MANA, 3600);
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 8000 })).toBeTruthy();
    const record = readSplitHold(BOOK.buyer, splitHoldLine(line.asset));
    expect(record?.txHash).toBeTruthy();
    const clock = vi.spyOn(Date, "now").mockReturnValue(Date.now() + SPLIT_HOLD_MS + 60_000);
    try {
      await checkAgain();
      expect(screen.getByText(/still confirming/)).toBeTruthy();
      expect(screen.queryByTestId("combined-confirm")).toBeNull();
      expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toMatchObject({ txHash: record!.txHash, attempt: record!.attempt });
    } finally {
      clock.mockRestore();
    }
  });

  it("two tabs paying at once: one takes the item's lock and sends, the other authorizes and sends nothing", async () => {
    const { demo, again } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    again();
    const [first, second] = screen.getAllByTestId("combined-confirm");
    await act(async () => {
      fireEvent.click(first!);
      fireEvent.click(second!);
    });
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    // The second finds the manager's grant lock taken (a sale takes it before its item's).
    expect(await screen.findByText(/is in progress in another window\. Try again in a moment\. Nothing was charged here\./)).toBeTruthy();
    expect(authorizations(demo)).toBe(1);
    expect(demo.relay.posts).toHaveLength(1);
  });

  it("without Web Locks nothing is authorized or sent: this browser can't make sure the split goes once", async () => {
    const { demo, unmount, again } = setup();
    unmount();
    demo.services.locks = null;
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    again();
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/can't make sure a purchase with Credits and MANA is sent only once/)).toBeTruthy();
    expect(authorizations(demo)).toBe(0);
    expect(demo.relay.posts).toHaveLength(0);
  });

  /** What another tab of this browser leaves when its split's answer is lost: held, no hash, its credit still spendable. */
  const holdFromAnotherTab = (line: ReturnType<typeof setup>["line"]) =>
    localStorage.setItem(
      `dcl-shop-buynow-split:v1:${BOOK.buyer.toLowerCase()}:${splitHoldLine(line.asset)}`,
      JSON.stringify({ at: Date.now(), txHash: null, chainId: CHAIN.POLYGON, creditId: OTHER, creditExpiresAt: Math.floor(Date.now() / 1000) + 300 }),
    );

  it("a hold one tab wrote stops the purchase in another: a fresh dialog, with a session of its own, shows it confirming and sends nothing", async () => {
    const { demo, line, unmount, again } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    demo.relay.state.mode = "fail-after-send";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    unmount();
    // Another tab: its session storage starts empty, this browser's local storage is the same one.
    sessionStorage.clear();
    demo.relay.state.mode = "ok";
    again();
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    expect(demo.relay.posts).toHaveLength(1);
    expect(authorizations(demo)).toBe(1);
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toMatchObject({ txHash: null, creditId: expect.any(String) });
  });

  it("a hold on the item through another listing (another surface may sell it through a creator's trade) stops this one too", async () => {
    const { demo, line, unmount, again } = setup();
    unmount();
    // This dialog buys the store's mint; the hold was written for the same item bought through a trade.
    expect(line.listing).toEqual({ acquisition: "store" });
    localStorage.setItem(
      `dcl-shop-buynow-split:v1:${BOOK.buyer.toLowerCase()}:${splitHoldLine(line.asset, { tradeId: "creator-trade" })}`,
      JSON.stringify({ at: Date.now(), attempt: "elsewhere", txHash: null, chainId: CHAIN.POLYGON, creditId: OTHER, creditExpiresAt: Math.floor(Date.now() / 1000) + 300 }),
    );
    again();
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    expect(authorizations(demo)).toBe(0);
  });

  it("a dialog already open when another tab's hold appears reads it again on Pay: shown confirming, nothing authorized or sent", async () => {
    const { demo, line } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    expect(screen.getByTestId("combined-confirm")).toBeEnabled();
    holdFromAnotherTab(line);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    expect(authorizations(demo)).toBe(0);
    expect(demo.relay.posts).toHaveLength(0);
  });

  it("a hold another tab wrote while this one approved is read under the item's lock: no credit is authorized, the other shown confirming", async () => {
    const { demo, line } = setup();
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    const approve = await screen.findByTestId("authorize-step-action");
    holdFromAnotherTab(line);
    await act(async () => fireEvent.click(approve));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    // The approval alone was relayed; no credit was reserved for a sale that was never going to be sent.
    expect(demo.relay.posts).toHaveLength(1);
    expect(relayedApproval(demo.relay.posts[0])).not.toBeNull();
    expect(authorizations(demo)).toBe(0);
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))?.creditId).toBe(OTHER);
  });

  it("a browser that cannot keep the record sends nothing: the credit is released and the buyer told why", async () => {
    const { demo, line } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    const setItem = Storage.prototype.setItem;
    const full = vi.spyOn(Storage.prototype, "setItem").mockImplementation(function (this: Storage, key: string, value: string) {
      if (key.startsWith("dcl-shop-buynow-split:")) throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
      setItem.call(this, key, value);
    });
    try {
      await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
      const told = "Your browser couldn't keep a record of this purchase, so it wasn't sent. Nothing was charged.";
      expect(await screen.findByText(told, {}, { timeout: 5000 })).toBeTruthy();
    } finally {
      full.mockRestore();
    }
    expect(demo.relay.posts).toHaveLength(0);
    expect(authorizations(demo)).toBe(1);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toBeNull();
    expect(screen.getByTestId("combined-confirm")).toBeEnabled();
  });

  it("a receipt read aborted after the relay answered the sale's hash keeps the sale pending: its credit kept, held on reopen, completed by its receipt", async () => {
    const { demo, line, unmount, again } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    const read = demo.chain.receipt;
    demo.chain.receipt = async () => {
      throw new DOMException("Aborted", "AbortError");
    };
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    expect(demo.shop.log.filter((r) => r.name === "cancelCreditAuthorizations")).toHaveLength(0);
    const held = readSplitHold(BOOK.buyer, splitHoldLine(line.asset));
    expect(held).toMatchObject({ txHash: expect.any(String), creditId: expect.any(String) });
    unmount();
    again();
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    // Its receipt can be read again, and it mined: Check again completes it, nothing sent twice.
    demo.chain.receipt = read;
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(await screen.findByText(/Purchase complete/)).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    expect(demo.shop.log.filter((r) => r.name === "cancelCreditAuthorizations")).toHaveLength(0);
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toBeNull();
  });

  it.each([
    ["still reserved (PENDING)", "PENDING"],
    ["not listed yet", null],
  ] as const)("its own bytes mined and reverted, its credit %s: held with no credit retired until the node settles it, and bought once", async (_, first) => {
    const credit = `0x${"a1".repeat(32)}`;
    const sibling = `0x${"ab".repeat(32)}`;
    const listed: { status: "PENDING" | "SETTLED" | null; txHash: string | null } = { status: first, txHash: null };
    const { demo, line } = setup(undefined, "reverted", MANA, undefined, {
      userPurchases: () => ({
        purchases: listed.status ? [{ id: `${credit}:0`, status: listed.status, usdCents: 50, createdAt: 1, txHash: listed.txHash }] : [],
        total: listed.status ? 1 : 0,
      }),
    });
    const cancels = () => demo.shop.log.filter((r) => r.name === "cancelCreditAuthorizations").length;
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    expect(cancels()).toBe(0);
    const held = readSplitHold(BOOK.buyer, splitHoldLine(line.asset));
    expect(held).toMatchObject({ txHash: expect.any(String), creditId: credit });
    // Checked again while the node still says so: nothing changes.
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toEqual(held);
    expect(cancels()).toBe(0);
    // A sibling invocation of the same terms had run: the node settles the credit by it.
    const [to, input] = (demo.relay.posts.at(-1) as { transactionData: { params: [string, Hex] } }).transactionData.params;
    demo.chain.transactions.set(sibling, { from: null, to, input });
    demo.chain.receipts.set(sibling, "success");
    Object.assign(listed, { status: "SETTLED", txHash: sibling });
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(cancels()).toBe(0);
    expect(demo.relay.posts).toHaveLength(1);
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toBeNull();
  });

  it.each([
    ["refuses", true],
    ["keeps without a word", false],
  ] as const)("a live purchase whose record the storage %s is not shown complete: held with its hash, and Check again finishes it once the record can go", async (_, throws) => {
    const { demo, line } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    const removeItem = Storage.prototype.removeItem;
    const stuck = vi.spyOn(Storage.prototype, "removeItem").mockImplementation(function (this: Storage, key: string) {
      if (!key.startsWith("dcl-shop-buynow-split:")) return removeItem.call(this, key);
      if (throws) throw new DOMException("The operation is insecure.", "SecurityError");
    });
    try {
      await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
      expect(await screen.findByTestId("combined-check-notice", {}, { timeout: 5000 })).toHaveTextContent("This browser couldn't update its record of this purchase. Check again in a moment.");
      expect(screen.queryByText(/Purchase complete/)).toBeNull();
      expect(screen.getByText(/still confirming/)).toBeTruthy();
      expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toMatchObject({ txHash: expect.any(String), creditId: expect.any(String) });
      expect(demo.shop.log.filter((r) => r.name === "cancelCreditAuthorizations")).toHaveLength(0);
    } finally {
      stuck.mockRestore();
    }
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toBeNull();
    expect(demo.relay.posts).toHaveLength(1);
  });

  it("a sibling the node settled, whose record the storage keeps, is held with the sibling's hash: Check again reads it at once, not the history", async () => {
    const credit = `0x${"a1".repeat(32)}`;
    const replayed = `0x${"ab".repeat(32)}`;
    const box: { demo: ReturnType<typeof setup>["demo"] | null } = { demo: null };
    const { demo, line } = setup(undefined, "reverted", MANA, undefined, {
      userPurchases: () => {
        // The very execution the relay was given, run again in another transaction, which mined: the node settled it.
        const [to, input] = (box.demo!.relay.posts.at(-1) as { transactionData: { params: [string, Hex] } }).transactionData.params;
        box.demo!.chain.transactions.set(replayed, { from: null, to, input });
        box.demo!.chain.receipts.set(replayed, "success");
        return { purchases: [{ id: `${credit}:0`, status: "SETTLED", usdCents: 50, createdAt: 1, txHash: replayed }], total: 1 };
      },
    });
    box.demo = demo;
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    const removeItem = Storage.prototype.removeItem;
    const stuck = vi.spyOn(Storage.prototype, "removeItem").mockImplementation(function (this: Storage, key: string) {
      if (!key.startsWith("dcl-shop-buynow-split:")) return removeItem.call(this, key);
    });
    try {
      await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
      expect(await screen.findByTestId("combined-check-notice", {}, { timeout: 5000 })).toHaveTextContent("couldn't update its record");
      expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))?.txHash).toBe(replayed);
    } finally {
      stuck.mockRestore();
    }
    const reads = demo.shop.log.filter((r) => r.name === "userPurchases").length;
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.shop.log.filter((r) => r.name === "userPurchases")).toHaveLength(reads);
  });

  it("a successful receipt for a transaction that is not this sale's, with nothing in the history, is held confirming with that hash, never complete", async () => {
    const { demo, line } = setup(undefined, undefined, MANA, undefined, { userPurchases: () => ({ purchases: [], total: 0 }) });
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    // What the chain shows at the relay's hash: another buyer's useCredits, mined.
    const read = demo.chain.transaction!;
    demo.chain.transaction = async (chainId, hash) => {
      const tx = await read(chainId, hash);
      if (!tx) return tx;
      const meta = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: tx.input });
      return { ...tx, input: encodeExecuteMetaTransaction("functionData", `0x${"cc".repeat(20)}`, meta.args[1] as Hex, `0x${"11".repeat(65)}`) };
    };
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    expect(screen.queryByText(/Purchase complete/)).toBeNull();
    const held = readSplitHold(BOOK.buyer, splitHoldLine(line.asset));
    expect(held).toMatchObject({ txHash: expect.any(String), creditId: expect.any(String) });
    expect(demo.chain.receipts.get(held!.txHash!.toLowerCase())).toBe("success");
    expect(demo.relay.posts).toHaveLength(1);
    expect(demo.shop.log.filter((r) => r.name === "cancelCreditAuthorizations")).toHaveLength(0);
  });

  it("a revert of its terms in bytes it did not dispatch (the relay's hash is the same call signed again) is not its own: held, its credit kept", async () => {
    const { demo, line } = setup(undefined, "reverted");
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    // What the chain shows at the relay's hash: the same useCredits, from the same buyer, under another signature.
    const read = demo.chain.transaction!;
    demo.chain.transaction = async (chainId, hash) => {
      const tx = await read(chainId, hash);
      if (!tx) return tx;
      const meta = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: tx.input });
      return { ...tx, input: encodeExecuteMetaTransaction("functionData", meta.args[0] as string, meta.args[1] as Hex, `0x${"22".repeat(65)}`) };
    };
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    expect(demo.shop.log.filter((r) => r.name === "cancelCreditAuthorizations")).toHaveLength(0);
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toMatchObject({ txHash: expect.any(String), creditId: expect.any(String) });
  });

  it("its own bytes reverted, then the same execution ran in another transaction the node settled: the purchase completes, its credit kept", async () => {
    const credit = `0x${"a1".repeat(32)}`;
    const replayed = `0x${"ab".repeat(32)}`;
    const box: { demo: ReturnType<typeof setup>["demo"] | null } = { demo: null };
    const { demo, line } = setup(undefined, "reverted", MANA, undefined, {
      userPurchases: () => {
        // The very execution the relay was given, run again in another transaction, which mined: the node settled it.
        const [to, input] = (box.demo!.relay.posts.at(-1) as { transactionData: { params: [string, Hex] } }).transactionData.params;
        box.demo!.chain.transactions.set(replayed, { from: null, to, input });
        box.demo!.chain.receipts.set(replayed, "success");
        return { purchases: [{ id: `${credit}:0`, status: "SETTLED", usdCents: 50, createdAt: 1, txHash: replayed }], total: 1 };
      },
    });
    box.demo = demo;
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    expect(demo.shop.log.filter((r) => r.name === "cancelCreditAuthorizations")).toHaveLength(0);
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toBeNull();
  });

  it("a dialog closed while its sale's own approval confirms sends no sale: only the approve went out, its credit is let go", async () => {
    const { demo, line, unmount } = setup();
    // The allowance reads enough when the purchase starts, then nothing once its credit is issued: the sale approves first.
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    const call = demo.chain.call.bind(demo.chain);
    let short = false;
    demo.chain.call = async (chainId, to, data) => {
      if (!short && data.startsWith("0xdd62ed3e") && demo.shop.log.some((r) => r.name === "authorizeCredit")) {
        short = true;
        demo.chain.allowances.delete(managerKey());
      }
      return call(chainId, to, data);
    };
    // The approve's receipt is held back until the dialog is gone.
    let land!: () => void;
    const landing = new Promise<void>((resolve) => (land = resolve));
    let waiting = false;
    const receipt = demo.chain.receipt.bind(demo.chain);
    demo.chain.receipt = async (...args: Parameters<typeof receipt>) => {
      if (!waiting && demo.relay.posts.length === 1) {
        waiting = true;
        await landing;
      }
      return receipt(...args);
    };
    act(() => {
      fireEvent.click(screen.getByTestId("combined-confirm"));
    });
    await waitFor(() => expect(waiting).toBe(true), { timeout: 5000 });
    unmount();
    await act(async () => {
      land();
      await new Promise((resolve) => setTimeout(resolve, 50));
    });
    // One POST, the approve's; one signature, the approve's; no sale kept, signed or sent; its credit given back.
    expect(demo.relay.posts.map(relayedApproval).filter((a) => a !== null)).toHaveLength(1);
    expect(demo.relay.posts).toHaveLength(1);
    expect(demo.wallet.requests.filter((r) => r.method === "eth_signTypedData_v4")).toHaveLength(1);
    expect(readSplitHold(BOOK.buyer, splitHoldLine(line.asset))).toBeNull();
    expect(demo.shop.log.filter((r) => r.name === "cancelCreditAuthorizations")).toHaveLength(1);
  });

  it("a dialog closed while the wallet signs its approval records and sends nothing: no grant left to reconcile, no POST", async () => {
    const { demo, unmount } = setup();
    // The wallet holds its signature until the dialog is gone.
    let sign!: () => void;
    const signing = new Promise<void>((resolve) => (sign = resolve));
    const request = demo.wallet.request.bind(demo.wallet);
    let asked = 0;
    demo.wallet.request = (async (args: Parameters<typeof request>[0]) => {
      if (args.method === "eth_signTypedData_v4") {
        asked++;
        await signing;
      }
      return request(args);
    }) as typeof demo.wallet.request;
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    const approve = await screen.findByTestId("authorize-step-action");
    act(() => {
      fireEvent.click(approve);
    });
    await waitFor(() => expect(asked).toBe(1));
    unmount();
    await act(async () => {
      sign();
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(demo.relay.posts).toHaveLength(0);
    expect(Object.keys(localStorage).filter((k) => k.startsWith("dcl-shop-exact-grant:"))).toEqual([]);
    expect(authorizations(demo)).toBe(0);
  });

  it("an approval that went through but didn't take effect (its nonce passed, the allowance short) says to try again, and a retry buys", async () => {
    const { demo, unmount, again } = setup();
    unmount();
    // The approve runs, but another call takes its nonce and the allowance is left short.
    const short = { on: true };
    const relayed = demo.services.fetchImpl!;
    demo.services.fetchImpl = (async (...args: Parameters<typeof fetch>) => {
      const answer = await relayed(...args);
      if (short.on) demo.chain.allowances.delete(managerKey());
      return answer;
    }) as typeof fetch;
    again();
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText("The approval didn't take effect. Try again.", {}, { timeout: 5000 })).toBeTruthy();
    expect(screen.queryByText(/still confirming/)).toBeNull();
    expect(authorizations(demo)).toBe(0);
    // Trying again signs anew, at the next nonce, and the purchase goes through.
    short.on = false;
    await act(async () => fireEvent.click(screen.getByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts.map(relayedApproval).filter((a) => a !== null)).toHaveLength(2);
  });

  it("an approval another window is running says so: this one asked the wallet nothing, so it was not sent", async () => {
    const { demo } = setup();
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    const approve = await screen.findByTestId("authorize-step-action");
    const auth = getManaSpendingAuthorization(CHAIN.POLYGON, requireContracts(CHAIN.POLYGON).creditsManager!.address);
    (demo.services.locks as ReturnType<typeof createFakeLocks>).held.add(exactGrantLock(auth, BOOK.buyer));
    await act(async () => fireEvent.click(approve));
    expect(await screen.findByText("A MANA approval is already in progress in another window. Try again in a moment.")).toBeTruthy();
    expect(screen.queryByText(/Your approval was sent/)).toBeNull();
    expect(demo.wallet.requests.filter((r) => r.method === "eth_signTypedData_v4" || r.method === "eth_sendTransaction")).toEqual([]);
    expect(demo.relay.posts).toHaveLength(0);
  });

  it("with the relay off, the approval says it can't be sent from here: nothing is signed or sent", async () => {
    const { demo } = setup();
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    const approve = await screen.findByTestId("authorize-step-action");
    (window as { __SHOP_CHAIN__?: unknown }).__SHOP_CHAIN__ = { gasless: false };
    try {
      await act(async () => fireEvent.click(approve));
      expect(await screen.findByText(/The MANA approval can't be sent from here right now, so nothing was sent\. Try again later\./)).toBeTruthy();
    } finally {
      delete (window as { __SHOP_CHAIN__?: unknown }).__SHOP_CHAIN__;
    }
    expect(demo.wallet.requests.filter((r) => r.method === "eth_signTypedData_v4" || r.method === "eth_sendTransaction")).toEqual([]);
    expect(demo.relay.posts).toHaveLength(0);
  });

  it("right after its own approval, the sale waits a moment for the grant's lock the approval let go, rather than calling it another window's", async () => {
    const { demo, unmount, again } = setup();
    unmount();
    const base = demo.services.locks as LockManager;
    const auth = getManaSpendingAuthorization(CHAIN.POLYGON, requireContracts(CHAIN.POLYGON).creditsManager!.address);
    const name = exactGrantLock(auth, BOOK.buyer);
    // The approval takes the lock first; the browser frees it a little late, so the sale's next two asks find it taken.
    let asked = 0;
    demo.services.locks = {
      query: base.query.bind(base),
      request: ((n: string, options: LockOptions, callback: LockGrantedCallback<unknown>) => {
        if (n === name && (++asked === 2 || asked === 3)) return Promise.resolve(callback(null));
        return base.request(n, options, callback);
      }) as LockManager["request"],
    } as LockManager;
    again();
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(asked).toBe(4);
  });

  it("without Web Locks no approval is asked for: nothing is signed, authorized or sent", async () => {
    const { demo, unmount, again } = setup();
    unmount();
    demo.services.locks = null;
    again();
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/can't make sure a purchase with Credits and MANA is sent only once/)).toBeTruthy();
    expect(screen.queryByTestId("authorize-step")).toBeNull();
    expect(demo.wallet.requests.filter((r) => r.method === "eth_signTypedData_v4" || r.method === "eth_sendTransaction")).toEqual([]);
    expect(authorizations(demo)).toBe(0);
    expect(demo.relay.posts).toHaveLength(0);
  });

  it("while another window holds the manager's grant lock, the sale stops before any credit: nothing authorized or sent", async () => {
    const { demo } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    const auth = getManaSpendingAuthorization(CHAIN.POLYGON, requireContracts(CHAIN.POLYGON).creditsManager!.address);
    (demo.services.locks as ReturnType<typeof createFakeLocks>).held.add(exactGrantLock(auth, BOOK.buyer));
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/is in progress in another window\. Try again in a moment\. Nothing was charged here\./)).toBeTruthy();
    expect(authorizations(demo)).toBe(0);
    expect(demo.relay.posts).toHaveLength(0);
  });

  it("an approval another window lost, which can still run, keeps the sale from spending an allowance another grant covers", async () => {
    const { demo } = setup();
    // Another grant covers the gap; the recorded approve (nonce 0, unspent) could still land after a sale spends it.
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    const auth = getManaSpendingAuthorization(CHAIN.POLYGON, requireContracts(CHAIN.POLYGON).creditsManager!.address);
    const mana = requireContracts(CHAIN.POLYGON).mana.address.toLowerCase();
    localStorage.setItem(
      `dcl-shop-exact-grant:v2:${BOOK.buyer.toLowerCase()}:${CHAIN.POLYGON}:${mana}:${auth.spenderAddress}`,
      JSON.stringify({
        at: Date.now(),
        attempt: "other",
        amount: (13n * 10n ** 17n).toString(),
        to: mana,
        data: encodeExecuteMetaTransaction(metaTxTargetFor(auth)!.metaTx, BOOK.buyer, encodeAuthorizationCall(auth, true, 13n * 10n ** 17n), `0x${"11".repeat(64)}1b`),
        nonce: "0",
      }),
    );
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    // Not ready: the approval step, whose retry within the hold sends nothing.
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/Your approval was sent and is still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    expect(authorizations(demo)).toBe(0);
    expect(demo.relay.posts).toHaveLength(0);
  });

  it("an approval whose answer was lost stays pending through Approve and its retry: no second grant, and no credit or sale until it lands", async () => {
    const { demo } = setup();
    demo.relay.state.mode = "fail-after-send";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    const pending = /Your approval was sent and is still confirming/;
    expect(await screen.findByText(pending, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    // It has not landed yet (the fake relay applied it; take back its allowance and its nonce): a retry grants nothing.
    demo.chain.allowances.delete(managerKey());
    demo.chain.nonces.delete(manaNonceKey());
    await act(async () => fireEvent.click(screen.getByTestId("authorize-step-action")));
    expect(await screen.findByText(pending, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    expect(demo.wallet.requests.some((r) => r.method === "eth_sendTransaction")).toBe(false);
    expect(authorizations(demo)).toBe(0);
    // It lands (its nonce is spent and the allowance set): the retry finds it, grants nothing more, and the purchase goes on.
    demo.chain.allowances.set(managerKey(), 13n * 10n ** 17n);
    demo.chain.nonces.set(manaNonceKey(), 1n);
    demo.relay.state.mode = "ok";
    await act(async () => fireEvent.click(screen.getByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts.map(relayedApproval).filter((a) => a !== null)).toHaveLength(1);
    expect(authorizations(demo)).toBe(1);
  });
});

describe("a held split with no transaction to read", () => {
  const CREDIT = `0x${"c1".repeat(32)}`;
  type Listing = (req: { query: URLSearchParams }) => unknown;
  /** Its terms, kept with the hold: what an execution must carry to count as this split. */
  const SALE: SplitSale = {
    buyer: BOOK.buyer.toLowerCase(),
    chainId: CHAIN.POLYGON,
    manager: requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase(),
    creditId: CREDIT,
    creditWei: MANA.toString(),
    gapWei: ((13n * MANA) / 10n).toString(),
    target: BOOK.store.contract.toLowerCase(),
    selector: "0x12345678",
    callData: "0xabcdef",
  };
  const SETTLED_BY = `0x${"ab".repeat(32)}`;
  /**
   * A split held for the store's Rainbow Hat, sent sixteen minutes ago, its credit expiring at `creditExpiresAt`. The node
   * lists its credit as `status` says; SETTLED by `SETTLED_BY`, which the chain shows as this split (`settledBy: "this"`,
   * the default), as another purchase of this credit ("other"), as another account's useCredits ("stranger") or not at all
   * ("none").
   */
  function heldSplit(
    status: { value: "PENDING" | "SETTLED" | "EXPIRED" | null },
    creditExpiresAt: number,
    opts: { userPurchases?: Listing; attempt?: string; txHash?: string; settledBy?: "this" | "other" | "stranger" | "none"; expiredSeenAt?: number; envelope?: Hex } = {},
  ) {
    const demo = createDemo({
      session: "web3",
      handlers: {
        userPurchases: opts.userPurchases ?? (() => ({
          // As the node lists it: the credit's salt and the line's position.
          purchases: status.value ? [{ id: `${CREDIT}:0`, status: status.value, usdCents: 50, createdAt: 1, txHash: status.value === "SETTLED" ? SETTLED_BY : null }] : [],
          total: status.value ? 1 : 0,
        })),
      },
    });
    const offer = storeMint(demo.session);
    const mixed = presentPayment(offer, demo.session, { creditsBalance: usdCents(50), report: () => {} }).options.find((o) => o.method === "mana")!.mixed as MixedPayment;
    const line = { asset: offer.asset, listing: { acquisition: "store" as const }, display: { name: "Rainbow Hat" } };
    const settledBy = opts.settledBy ?? "this";
    if (settledBy !== "none") {
      const settling = { this: splitSaleTransaction(SALE), other: splitSaleTransaction({ ...SALE, callData: "0x0bad" }), stranger: splitSaleTransaction(SALE, { user: `0x${"cc".repeat(20)}` }) };
      demo.chain.transactions.set(SETTLED_BY, settling[settledBy]);
      demo.chain.receipts.set(SETTLED_BY, "success");
    }
    // Sent sixteen minutes ago, past the hold; the relay lost its answer, so no hash (unless one is given).
    localStorage.setItem(
      `dcl-shop-buynow-split:v1:${BOOK.buyer.toLowerCase()}:${splitHoldLine(line.asset)}`,
      JSON.stringify({
        at: Date.now() - 16 * 60 * 1000,
        ...(opts.attempt ? { attempt: opts.attempt } : {}),
        txHash: opts.txHash ?? null,
        chainId: CHAIN.POLYGON,
        creditId: CREDIT,
        creditExpiresAt,
        sale: opts.envelope ? { ...SALE, envelope: opts.envelope } : SALE,
        ...(opts.expiredSeenAt !== undefined ? { expiredSeenAt: opts.expiredSeenAt } : {}),
      }),
    );
    render(
      <DemoShell demo={demo} path="/shop/item/x/7">
        <CombinedBuy line={line} offer={offer} mixed={mixed} onClose={() => {}} portal={false} />
      </DemoShell>,
    );
    return Object.assign(demo, { lineId: splitHoldLine(line.asset) });
  }
  const nowSeconds = () => Math.floor(Date.now() / 1000);

  it("is not let go by time while its credit can still be spent", async () => {
    heldSplit({ value: "EXPIRED" }, nowSeconds() + 300);
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.getByTestId("combined-check-again")).toBeDisabled();
  });

  it("once its credit expired, the node's record decides: still reserved keeps it held, released lets the item go", async () => {
    const status: { value: "PENDING" | "SETTLED" | "EXPIRED" | null } = { value: "PENDING" };
    const demo = heldSplit(status, nowSeconds() - 1);
    const check = await screen.findByTestId("combined-check-again");
    expect(check).toBeEnabled();
    await act(async () => fireEvent.click(check));
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    status.value = null;
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    // Released unspent: seen so, it is let go only once that has lasted the whole grace.
    status.value = "EXPIRED";
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(await screen.findByTestId("combined-check-notice")).toHaveTextContent(/expired unspent/);
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    const clock = vi.spyOn(Date, "now").mockReturnValue(Date.now() + SPLIT_EXPIRY_CONFIRM_MS + 1_000);
    try {
      await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
      expect(await screen.findByTestId("combined-confirm")).toBeEnabled();
    } finally {
      clock.mockRestore();
    }
    expect(demo.shop.log.filter((r) => r.name === "userPurchases")).toHaveLength(4);
  });

  it("a credit the node says this split spent completes the purchase, never offering it again", async () => {
    heldSplit({ value: "SETTLED" }, nowSeconds() - 1);
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    expect(await screen.findByText(/Purchase complete/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
  });

  it("a credit the node says another purchase spent: nothing was bought here, nothing to give back, and the item is let go", async () => {
    const demo = heldSplit({ value: "SETTLED" }, nowSeconds() - 1, { settledBy: "other" });
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    expect(await screen.findByText("The Credits this purchase held were spent on another purchase, so nothing was bought here.")).toBeTruthy();
    expect(screen.queryByText(/Purchase complete/)).toBeNull();
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
    expect(readSplitHold(BOOK.buyer, demo.lineId)).toBeNull();
  });

  it("SETTLED by a transaction that doesn't spend this credit for this buyer (another account's) says nothing: held, never spent elsewhere", async () => {
    const demo = heldSplit({ value: "SETTLED" }, nowSeconds() - 1, { settledBy: "stranger" });
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    await waitFor(() => expect(demo.shop.log.some((r) => r.name === "userPurchases")).toBe(true));
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByText(/spent on another purchase/)).toBeNull();
    expect(readSplitHold(BOOK.buyer, demo.lineId)).not.toBeNull();
  });

  it("SETTLED alone, with no transaction to read, proves the credit was spent, not on what: the split stays held", async () => {
    const demo = heldSplit({ value: "SETTLED" }, nowSeconds() - 1, { settledBy: "none" });
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    await waitFor(() => expect(demo.shop.log.some((r) => r.name === "userPurchases")).toBe(true));
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, demo.lineId)).not.toBeNull();
  });

  it.each([
    ["its terms signed again (another envelope)", false],
    ["the very bytes it dispatched", true],
  ] as const)("Check again on its hash mined and reverted as %s, its credit still reserved: held, no credit retired", async (_, own) => {
    const HASH = `0x${"5a".repeat(32)}`;
    const mine = splitSaleTransaction(SALE);
    const demo = heldSplit({ value: "PENDING" }, nowSeconds() + 300, { attempt: "a", txHash: HASH, envelope: splitEnvelope(mine.input), settledBy: "none" });
    demo.chain.transactions.set(HASH, own ? mine : splitSaleTransaction(SALE, { signature: `0x${"22".repeat(65)}` }));
    demo.chain.receipts.set(HASH, "reverted");
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    await waitFor(() => expect(demo.shop.log.some((r) => r.name === "userPurchases")).toBe(true));
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByText(/failed on-chain/)).toBeNull();
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
    expect(readSplitHold(BOOK.buyer, demo.lineId)).toMatchObject({ attempt: "a", txHash: HASH });
  });

  it.each([
    ["refuses", true],
    ["keeps without a word", false],
  ] as const)("an outcome whose record the storage %s is not applied: Check again says so, and completes once the storage recovers", async (_, throws) => {
    const demo = heldSplit({ value: "SETTLED" }, nowSeconds() - 1, { attempt: "a" });
    const removeItem = Storage.prototype.removeItem;
    const refused = vi.spyOn(Storage.prototype, "removeItem").mockImplementation(function (this: Storage, key: string) {
      if (!key.startsWith("dcl-shop-buynow-split:")) return removeItem.call(this, key);
      if (throws) throw new DOMException("The operation is insecure.", "SecurityError");
    });
    try {
      await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
      expect(await screen.findByTestId("combined-check-notice")).toHaveTextContent("This browser couldn't update its record of this purchase. Check again in a moment.");
      expect(screen.queryByText(/Purchase complete/)).toBeNull();
      expect(screen.getByText(/still confirming/)).toBeTruthy();
      expect(readSplitHold(BOOK.buyer, demo.lineId)).toMatchObject({ attempt: "a" });
    } finally {
      refused.mockRestore();
    }
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, demo.lineId)).toBeNull();
  });

  it("after another surface's dispatch, a FOREIGN hash leaves its unresolved attempt held on the web", async () => {
    // The in-world Shop sent this split and recorded a hash the chain shows as someone else's useCredits.
    const FOREIGN = `0x${"f0".repeat(32)}`;
    const demo = heldSplit({ value: "PENDING" }, nowSeconds() - 1, { attempt: "play-attempt", txHash: FOREIGN });
    demo.chain.transactions.set(FOREIGN, splitSaleTransaction(SALE, { user: `0x${"cc".repeat(20)}` }));
    demo.chain.receipts.set(FOREIGN, "success");
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    await waitFor(() => expect(demo.shop.log.some((r) => r.name === "userPurchases")).toBe(true));
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    expect(screen.queryByText(/Purchase complete/)).toBeNull();
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, demo.lineId)).toMatchObject({ attempt: "play-attempt", txHash: FOREIGN });
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
  });

  it("a stale outcome never wipes a newer attempt's record: another tab settled this one and sent anew while Check again read", async () => {
    const newer = { at: Date.now(), attempt: "newer", txHash: null, chainId: CHAIN.POLYGON, creditId: `0x${"0e".repeat(32)}`, creditExpiresAt: nowSeconds() + 300 };
    let key = "";
    const demo = heldSplit({ value: null }, nowSeconds() - 1, {
      attempt: "older",
      userPurchases: () => {
        // While this tab waits on the node, another one settles the older attempt and sends a new split of the item.
        localStorage.setItem(key, JSON.stringify(newer));
        return { purchases: [{ id: `${CREDIT}:0`, status: "EXPIRED", usdCents: 50, createdAt: 1, txHash: null }], total: 1 };
      },
    });
    key = `dcl-shop-buynow-split:v1:${BOOK.buyer.toLowerCase()}:${demo.lineId}`;
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    await waitFor(() => expect(demo.shop.log.some((r) => r.name === "userPurchases")).toBe(true));
    // The older attempt's outcome (released) is not applied to the newer one: it stays, and is what the dialog shows.
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeDisabled());
    expect(readSplitHold(BOOK.buyer, demo.lineId)).toMatchObject({ attempt: "newer" });
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
  });

  it("a receipt read that fails is no answer: once its credit expired, the node's record of it decides, spent or released", async () => {
    for (const [value, settled] of [
      ["SETTLED", true],
      ["EXPIRED", false],
    ] as const) {
      const own = `0x${"5a".repeat(32)}`;
      // EXPIRED seen so for the whole grace already, so it decides on this click.
      const demo = heldSplit({ value }, nowSeconds() - 1, { txHash: own, expiredSeenAt: Date.now() - SPLIT_EXPIRY_CONFIRM_MS - 1_000 });
      // Its own receipt can't be read; the node's evidence (the SETTLED transaction) can.
      const read = demo.chain.receipt;
      demo.chain.receipt = async (chainId, hash) => {
        if (hash.toLowerCase() === own) throw new Error("the RPC is down");
        return read(chainId, hash);
      };
      await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
      if (settled) expect(await screen.findByText(/Purchase complete/)).toBeTruthy();
      else expect(await screen.findByTestId("combined-confirm")).toBeEnabled();
      expect(demo.shop.log.some((r) => r.name === "userPurchases")).toBe(true);
      expect(readSplitHold(BOOK.buyer, demo.lineId)).toBeNull();
      cleanup();
      localStorage.clear();
    }
  });

  it("a receipt read that fails, and a credit the node does not list, keep the split held", async () => {
    const demo = heldSplit({ value: null }, nowSeconds() - 1, { txHash: `0x${"5a".repeat(32)}` });
    demo.chain.receipt = async () => {
      throw new Error("the RPC is down");
    };
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    await waitFor(() => expect(demo.shop.log.some((r) => r.name === "userPurchases")).toBe(true));
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, demo.lineId)).not.toBeNull();
  });

  it("Check again says so when another window holds the item, and keeps the hold", async () => {
    const demo = heldSplit({ value: "EXPIRED" }, nowSeconds() - 1);
    (demo.services.locks as ReturnType<typeof createFakeLocks>).held.add(`dcl-shop-buynow-split:${BOOK.buyer.toLowerCase()}:${demo.lineId}`);
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    expect(await screen.findByTestId("combined-check-notice")).toHaveTextContent("This purchase is being paid or checked in another window. Check again in a moment.");
    expect(readSplitHold(BOOK.buyer, demo.lineId)).not.toBeNull();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
  });

  it("pages the history by the rows the server sent: a server that sends fewer than asked, some of them rows the decoder drops, does not skip the rest", async () => {
    // Every 25th line has a status this page doesn't know: decoded, the first page holds 144 lines of the 150 sent.
    const lines = Array.from({ length: 400 }, (_, n) => ({ id: `0x${n.toString(16).padStart(64, "0")}:0`, status: n % 25 === 0 ? "SOMETHING_NEW" : "SETTLED", usdCents: 10, createdAt: 1, txHash: null }));
    lines[170] = { id: `${CREDIT}:0`, status: "EXPIRED", usdCents: 50, createdAt: 1, txHash: null };
    const demo = heldSplit({ value: null }, nowSeconds() - 1, {
      expiredSeenAt: Date.now() - SPLIT_EXPIRY_CONFIRM_MS - 1_000,
      // It sends 150 rows a page whatever was asked.
      userPurchases: ({ query }) => {
        const offset = Number(query.get("offset") ?? 0);
        return { purchases: lines.slice(offset, offset + 150), total: lines.length };
      },
    });
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    expect(await screen.findByTestId("combined-confirm")).toBeEnabled();
    expect(demo.shop.log.filter((r) => r.name === "userPurchases").map((r) => r.query.get("offset"))).toEqual(["0", "150"]);
  });

  it("pages the history by the rows the server sent, so rows the decoder drops neither repeat a page nor end it early", async () => {
    const lines = Array.from({ length: 450 }, (_, n) => ({ id: `0x${n.toString(16).padStart(64, "0")}:0`, status: n % 20 === 0 ? "SOMETHING_NEW" : "SETTLED", usdCents: 10, createdAt: 1, txHash: null }));
    lines[420] = { id: `${CREDIT}:0`, status: "EXPIRED", usdCents: 50, createdAt: 1, txHash: null };
    const demo = heldSplit({ value: null }, nowSeconds() - 1, {
      expiredSeenAt: Date.now() - SPLIT_EXPIRY_CONFIRM_MS - 1_000,
      userPurchases: ({ query }) => {
        const offset = Number(query.get("offset") ?? 0);
        const limit = Number(query.get("limit") ?? 50);
        return { purchases: lines.slice(offset, offset + limit), total: lines.length };
      },
    });
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    expect(await screen.findByTestId("combined-confirm")).toBeEnabled();
    expect(demo.shop.log.filter((r) => r.name === "userPurchases").map((r) => r.query.get("offset"))).toEqual(["0", "200", "400"]);
  });

  it("finds its credit past the newest lines: the history is read page by page until it is found", async () => {
    const lines = Array.from({ length: 450 }, (_, n) => ({ id: `0x${n.toString(16).padStart(64, "0")}:0`, status: "SETTLED", usdCents: 10, createdAt: 1, txHash: null }));
    lines[420] = { id: `${CREDIT}:0`, status: "EXPIRED", usdCents: 50, createdAt: 1, txHash: null };
    const demo = heldSplit({ value: null }, nowSeconds() - 1, {
      expiredSeenAt: Date.now() - SPLIT_EXPIRY_CONFIRM_MS - 1_000,
      userPurchases: ({ query }) => {
        const offset = Number(query.get("offset") ?? 0);
        const limit = Number(query.get("limit") ?? 50);
        return { purchases: lines.slice(offset, offset + limit), total: lines.length };
      },
    });
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    expect(await screen.findByTestId("combined-confirm")).toBeEnabled();
    const pages = demo.shop.log.filter((r) => r.name === "userPurchases").map((r) => [r.query.get("offset"), r.query.get("limit")]);
    expect(pages).toEqual([
      ["0", "200"],
      ["200", "200"],
      ["400", "200"],
    ]);
  });

  it("a credit the history does not list, read to its end, keeps the split held", async () => {
    const lines = Array.from({ length: 250 }, (_, n) => ({ id: `0x${n.toString(16).padStart(64, "0")}:0`, status: "SETTLED", usdCents: 10, createdAt: 1, txHash: null }));
    const demo = heldSplit({ value: null }, nowSeconds() - 1, {
      userPurchases: ({ query }) => {
        const offset = Number(query.get("offset") ?? 0);
        return { purchases: lines.slice(offset, offset + 200), total: lines.length };
      },
    });
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    await waitFor(() => expect(demo.shop.log.filter((r) => r.name === "userPurchases")).toHaveLength(2));
    await waitFor(() => expect(screen.getByTestId("combined-check-again")).toBeEnabled());
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    expect(readSplitHold(BOOK.buyer, demo.lineId)).not.toBeNull();
  });
});

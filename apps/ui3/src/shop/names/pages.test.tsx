import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Hex } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LAND_NETWORKS } from "../land/contracts";
import { erc20Abi } from "../land/evm";
import { NET, nameResult } from "../land/fixtures";
import { fakeWallet, stubClient, type Answer } from "../land/testkit";
import { ShopHarness, type SessionKind } from "../land/testkit-react";
import { CHAIN } from "../model/chain";
import { controllerAbi, NAME_PRICE_WEI, registrarAbi, useCreditsAbi } from "./names";
import NamesHub from "./NamesHub";
import { interimNameOffer } from "./offer";
import { RESUME_NAME_KEY } from "./resume";
import type { EvmAddress } from "../model/address";
import type { PurchaseOffer } from "../model/purchase";
import { nameRegistration } from "../payment/fixtures";

const MANA = 10n ** 18n;
const BUYER = "0x0000000000000000000000000000000000000b0b";
const MAINNET = LAND_NETWORKS[CHAIN.ETHEREUM]!;
const timing = { wait: { pollMs: 0, timeoutMs: 50 }, acrossPoll: { intervalMs: 0, maxAttempts: 1, fetchImpl: async () => new Response(JSON.stringify({ status: "filled", actionsSucceeded: true, fillTx: "0xfill" })) } };

type Setup = {
  session: SessionKind;
  available?: boolean;
  creditsQuote?: string | null;
  balanceCents?: number;
  ethMana?: bigint;
  polygonMana?: bigint;
  chainId?: number;
  extra?: Record<string, Answer>;
  host?: Record<string, unknown>;
  path?: string;
};

function setup(o: Setup) {
  const wallet = fakeWallet({
    account: BUYER,
    chainId: o.chainId ?? 1,
    reads: [
      { abi: registrarAbi, name: "available", answer: () => o.available ?? true },
      { abi: registrarAbi, name: "ownerOf", answer: () => BUYER },
      { abi: erc20Abi, name: "balanceOf", answer: (_args, to) => (to === MAINNET.mana ? (o.ethMana ?? 500n * MANA) : (o.polygonMana ?? 0n)) },
      { abi: erc20Abi, name: "allowance", answer: () => 0n },
      { abi: controllerAbi, name: "PRICE", answer: () => NAME_PRICE_WEI },
      { abi: controllerAbi, name: "acceptedToken", answer: () => MAINNET.mana },
      { abi: controllerAbi, name: "registrar", answer: () => MAINNET.nameRegistrar },
    ],
    writes: [controllerAbi, useCreditsAbi],
  });
  const reader = (chainId: number) => ({ chainId, call: (to: string, data: Hex) => wallet.request({ method: "eth_call", params: [{ to, data }] }) as Promise<Hex> });
  const client = stubClient({
    "POST /shop-api/v1/credit-quotes": { body: { items: [], amounts: [o.creditsQuote === undefined ? "500" : o.creditsQuote] } },
    "GET /shop-api/v1/users/0x0000000000000000000000000000000000000b0b/credits": { body: { credits: [], totalCredits: 0, totals: { expiring: 0, nonExpiring: 0 }, usd: { balanceCents: o.balanceCents ?? 10_000, credits: (o.balanceCents ?? 10_000) / 10 } } },
    "GET /shop-api/v1/packs": { body: { packs: [{ id: "p1", usd: 10, credits: 100, recommended: false, order: 1 }, { id: "p2", usd: 50, credits: 500, recommended: true, order: 2 }] } },
    ...o.extra,
  });
  const offerSource = interimNameOffer({ client, reader: reader(1), chainId: 1 });
  render(
    <ShopHarness session={o.session} account={BUYER} provider={o.session === "web3" ? wallet : null} path={o.path ?? "/shop/names"} route="/shop/names" host={o.host}>
      <NamesHub client={client} offerSource={offerSource} loadBanned={async () => ["banned"]} debounceMs={0} modal={{ readerFor: reader, portal: false, timing }} />
    </ShopHarness>,
  );
  return { wallet, client };
}

const type = (value: string) => fireEvent.change(screen.getByRole("textbox", { name: "Search for a NAME" }), { target: { value } });

beforeEach(() => sessionStorage.clear());
afterEach(() => vi.restoreAllMocks());

describe("NAME registration search", () => {
  it("sanitizes input and explains invalid, banned and taken NAMEs; a taken one points to the resale browse", async () => {
    setup({ session: "web2" });
    type("a");
    expect(await screen.findByText("NAME too short: NAMEs must be at least 2 characters long.")).toBeInTheDocument();
    type("banned");
    expect(await screen.findByText("This NAME is not available.")).toBeInTheDocument();
    type("Na cho!");
    expect(screen.getByRole("textbox", { name: "Search for a NAME" })).toHaveValue("Nacho");
  });

  it("links a taken NAME to its resale search", async () => {
    setup({ session: "web2", available: false });
    type("taken");
    expect(await screen.findByRole("link", { name: "Make offer" })).toHaveAttribute("href", "/shop/names?section=resale&search=taken");
    expect(screen.getByRole("button", { name: "Claim name" })).toBeDisabled();
  });

  it("prices an available NAME in credits for everyone", async () => {
    setup({ session: "web2" });
    type("Nacho");
    expect(await screen.findByText("Nacho is available!")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Claim name for 500 Credits" })).toBeEnabled();
    expect(screen.queryByText(/MANA from your wallet/)).toBeNull();
  });

  it("an offer the rule judges a resale is a taken NAME: no credit price, no credits wording, a link to the resale browse", async () => {
    const resale: PurchaseOffer = { ...nameRegistration({ kind: "web3", wallet: BUYER as EvmAddress }), sale: "secondary" };
    vi.spyOn(console, "warn").mockImplementation(() => {});
    render(
      <ShopHarness session="web3" account={BUYER} provider={null} path="/shop/names" route="/shop/names">
        <NamesHub offerSource={async () => resale} loadBanned={async () => []} debounceMs={0} />
      </ShopHarness>,
    );
    type("Nacho");
    expect(await screen.findByRole("link", { name: "Make offer" })).toHaveAttribute("href", "/shop/names?section=resale&search=Nacho");
    expect(screen.getByRole("button", { name: "Claim name" })).toBeDisabled();
    expect(screen.queryByText(/Credits/)).toBeNull();
  });

  it("sends a signed-out visitor to sign in", async () => {
    const onSignIn = vi.fn();
    setup({ session: "signedOut", host: { onSignIn } });
    type("Nacho");
    fireEvent.click(await screen.findByRole("button", { name: "Claim name for 500 Credits" }));
    expect(onSignIn).toHaveBeenCalled();
  });

  it("re-opens the buy dialog on the NAME a top-up was started for", async () => {
    sessionStorage.setItem(RESUME_NAME_KEY, "Nacho");
    setup({ session: "web2" });
    expect(await screen.findByRole("dialog", { name: "Buy Nacho.dcl.eth" })).toBeInTheDocument();
    expect(sessionStorage.getItem(RESUME_NAME_KEY)).toBeNull();
  });
});

describe("the NAME buy dialog", () => {
  const open = async () => {
    type("Nacho");
    await screen.findByText("Nacho is available!");
    fireEvent.click(screen.getByRole("button", { name: /^Claim/ }));
    return screen.findByRole("dialog", { name: "Buy Nacho.dcl.eth" });
  };

  it("gives a web3 wallet credits first and selected, MANA second", async () => {
    setup({ session: "web3" });
    const dialog = await open();
    const radios = await within(dialog).findAllByRole("radio");
    expect(radios.map((r) => r.closest("label")!.textContent)).toEqual([expect.stringContaining("Credits"), expect.stringContaining("MANA on Ethereum")]);
    expect(radios[0]).toBeChecked();
  });

  it("offers a web2 account credits only, straight to the confirmation", async () => {
    setup({ session: "web2" });
    const dialog = await open();
    expect(within(dialog).queryByRole("radio")).toBeNull();
    expect(within(dialog).getByRole("textbox", { name: "Re-enter the NAME" })).toBeInTheDocument();
    expect(dialog.textContent).not.toMatch(/MANA/);
  });

  it("short on credits is an explicit top-up and keeps credits selected, never moving to MANA", async () => {
    setup({ session: "web3", balanceCents: 1_000 });
    const dialog = await open();
    await within(dialog).findByText("Insufficient Credits.", { exact: false });
    expect(within(dialog).getAllByRole("radio")[0]).toBeChecked();
    expect(within(dialog).getByText(/You need/)).toHaveTextContent("You need 400 more Credits to purchase this NAME.");
    expect(within(await within(dialog).findByRole("group", { name: "Buy Credits" })).getByRole("radio", { name: /500 Credits/ })).toBeChecked();
    expect(within(dialog).getByRole("button", { name: "Continue" })).toBeDisabled();
  });

  it("starts a pack checkout for the shortfall and stashes the NAME to resume", async () => {
    const assign = vi.fn();
    vi.stubGlobal("location", { ...window.location, assign });
    setup({ session: "web2", balanceCents: 1_000, extra: { "POST /shop-api/v1/pack-orders": { body: { orderId: "o1", url: "https://checkout.stripe.test/o1" } } } });
    const dialog = await open();
    fireEvent.click(await within(dialog).findByRole("button", { name: "Buy Credits" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://checkout.stripe.test/o1"));
    expect(sessionStorage.getItem(RESUME_NAME_KEY)).toBe("Nacho");
    vi.unstubAllGlobals();
  });

  it("in the iOS app's web view states the shortfall but sells no pack", async () => {
    setup({ session: "web2", balanceCents: 1_000, path: "/shop/names?view=mobile-iap" });
    const dialog = await open();
    expect(await within(dialog).findByText(/You need/)).toHaveTextContent("You need 400 more Credits");
    expect(within(dialog).queryByRole("button", { name: "Buy Credits" })).toBeNull();
  });

  it("keeps an unavailable credits price as the main action, with the reason; MANA stays a choice for web3", async () => {
    setup({ session: "web3", creditsQuote: null });
    const dialog = await open();
    expect(await within(dialog).findByText("The Credits price is unavailable right now: the MANA price feed is not answering. Try again in a moment.")).toBeInTheDocument();
    const radios = within(dialog).getAllByRole("radio");
    expect(radios[0]).toBeChecked();
    expect(within(dialog).getByRole("button", { name: "Continue" })).toBeDisabled();
    fireEvent.click(radios[1]!);
    expect(within(dialog).getByRole("button", { name: "Continue" })).toBeEnabled();
  });

  it("registers with MANA through the registrar on Ethereum after the re-entry gate", async () => {
    const { wallet } = setup({ session: "web3" });
    const dialog = await open();
    fireEvent.click(await within(dialog).findByRole("radio", { name: /MANA on Ethereum/ }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Continue" }));
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Re-enter the NAME" }), { target: { value: "nacho" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Buy" }));
    expect(await screen.findByRole("dialog", { name: "Purchase complete!" })).toBeInTheDocument();
    expect(wallet.sent.map((t) => [t.to, t.fn])).toEqual([
      [MAINNET.mana, "approve"],
      [MAINNET.nameController, "register"],
    ]);
  });

  it("asks a web3 wallet on Polygon to switch to Ethereum before a MANA registration", async () => {
    const { wallet } = setup({ session: "web3", chainId: 137 });
    const dialog = await open();
    fireEvent.click(await within(dialog).findByRole("radio", { name: /MANA on Ethereum/ }));
    fireEvent.click(await within(dialog).findByRole("button", { name: "Switch to Ethereum" }));
    await waitFor(() => expect(wallet.chainId).toBe(1));
  });

  it("registers with credits: the quote is authorized for the NAME and useCredits goes to the credit's manager", async () => {
    const credit = { id: `0x${"ab".repeat(32)}`, amount: "102000000000000000000", availableAmount: "102000000000000000000", expiresAt: 1, signature: "0x5151", contract: "0x000000000000000000000000000000000000c0de" };
    const { wallet, client } = setup({
      session: "web3",
      chainId: 137,
      extra: {
        "GET /shop-api/v1/names/credit-route": { body: { externalCall: { target: "0x000000000000000000000000000000000000e0e0", selector: "0x11223344", data: "0x", expiresAt: 1, salt: "0x01" }, customExternalCallSignature: "0x", quoteId: "q", estimatedRouteDuration: 1, fromChainId: "137", toChainId: "1" } },
        "POST /shop-api/v1/credit-authorizations": { body: { credit, maxCreditedValue: credit.amount, usdCents: 5000, oracleRate: "1" } },
        "POST /shop-api/v1/credit-authorizations/submitted": { body: { recorded: 1 } },
      },
    });
    const dialog = await open();
    fireEvent.click(await within(dialog).findByRole("button", { name: "Continue" }));
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Re-enter the NAME" }), { target: { value: "Nacho" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Buy" }));
    expect(await screen.findByRole("dialog", { name: "Purchase complete!" })).toBeInTheDocument();
    expect(wallet.sent[0]).toMatchObject({ to: credit.contract, fn: "useCredits" });
    expect(JSON.parse(client.requests.find((r) => r.url.endsWith("/credit-authorizations"))!.body!)).toMatchObject({ usdPriceCents: 5000, name: "Nacho" });
  });

  it("shows NAME credits being closed with the reason, and lets a web3 buyer choose MANA themselves", async () => {
    setup({ session: "web3", chainId: 137, extra: { "GET /shop-api/v1/names/credit-route": { status: 404, body: { error: "Name registration is not enabled", code: "NAMES_DISABLED" } } } });
    const dialog = await open();
    fireEvent.click(await within(dialog).findByRole("button", { name: "Continue" }));
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Re-enter the NAME" }), { target: { value: "Nacho" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Buy" }));
    expect(await screen.findByText("Registering a NAME with Credits is not available right now. Nothing was charged.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    const again = await screen.findByRole("dialog", { name: "Buy Nacho.dcl.eth" });
    expect(within(again).getAllByRole("radio")[0]).toBeChecked();
    expect(within(again).getByText(/You can still register it with MANA/)).toBeInTheDocument();
  });

  it("offers the mixed rail only inside MANA, from the whole credits held plus Polygon MANA", async () => {
    setup({ session: "web3", balanceCents: 3_000, polygonMana: 1_000n * MANA });
    const dialog = await open();
    const mixed = await within(dialog).findByRole("radio", { name: /Credits and MANA/ });
    expect(mixed.closest("label")!.textContent).toContain("300 Credits + 40 MANA");
    expect(within(dialog).getAllByRole("radio")[0]).toBeChecked();
  });
});

describe("NAMEs being resold", () => {
  it("lists NAME resales with their MANA price and links to the token page", async () => {
    const client = stubClient({ "GET /shop-api/v1/nfts": { body: { data: [nameResult("Nacho", { price: (250n * MANA).toString() })], total: 1 } } });
    render(
      <ShopHarness session="web2" path="/shop/names?section=resale" route="/shop/names">
        <NamesHub client={client} />
      </ShopHarness>,
    );
    const card = await screen.findByRole("link", { name: "Nacho.dcl.eth, 250 MANA" });
    expect(card).toHaveAttribute("href", `/shop/token/${NET.nameRegistrar}/1`);
    const q = new URL(client.requests[0]!.url).searchParams;
    expect([q.get("category"), q.get("isOnSale")]).toEqual(["ens", "true"]);
    expect(screen.getByText(/This is a resale: the seller is paid in MANA\./)).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/credit/i);
  });

  it("says the index is catching up instead of an empty market", async () => {
    const client = stubClient({ "GET /shop-api/v1/nfts": { status: 503, body: { error: "Service Unavailable", message: "nft-indexer is catching up" } } });
    render(
      <ShopHarness session="web2" path="/shop/names?section=resale" route="/shop/names">
        <NamesHub client={client} />
      </ShopHarness>,
    );
    expect(await screen.findByText(/nft-indexer is catching up.*not an empty market/)).toBeInTheDocument();
  });

});

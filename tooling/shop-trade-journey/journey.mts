// The Shop's trade loop, end to end on Decentraland's testnet: two real wallets drive the real UI of a
// local stack in the `testnet` profile, and every signature and transaction lands on Amoy.
//
//   node journey.mts [--primary] --preflight   the stack, sign-in and the wallets' balances; signs nothing on chain
//   node journey.mts                           the trade loop (README: what each step costs and who pays)
//   node journey.mts --primary                 the primary loop: a creator's discount, bought with MANA and credits
//
// A complete trade loop leaves the token where it started, so it can run again: the seller lists,
// reprices and cancels; the buyer bids, cancels and bids again; the seller accepts (the buyer now owns
// the token); the buyer lists it and the seller buys it back with MANA through the cart.
//
// The primary loop: the seller, creator of E2E_ITEM's collection, starts a discount from My Store; the
// buyer sees the lower price, buys one with MANA, tops up test credits with the mock card and buys one
// with credits; the seller ends the discount. It mints two units each run, so it runs while supply lasts.
import { mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createPublicClient, erc721Abi, erc20Abi, formatEther, http, isAddress, keccak256, parseAbi, parseEther, toHex, zeroAddress, type Hex } from "viem";
import { generatePrivateKey } from "viem/accounts";
import { polygonAmoy } from "viem/chains";
import type { Browser, BrowserContext, Page, Response } from "playwright-core";
import { launch } from "./lib/browser.mts";
import { journeyEnv, parseDiscount, parseItem, parseToken, parseTopup } from "./lib/env.mts";
import { signedPost } from "./lib/signed.mts";
import { chargedMana, collectionInRoot, couponData, couponProofs, settledCall, soldItem } from "./lib/settlement.mts";
import { answer, DEFAULT_RPC, keyFrom, PAGE_PROVIDER, testnetWallet, type TestnetWallet } from "./lib/wallet.mts";

const AMOY_MANA = "0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0" as const;
const RELAYED = 4 * 60_000; // a relayed or mined step: signature, relay, inclusion, the Shop's indexing
const OUT = fileURLToPath(new URL("../../state/shop-trade-journey/", import.meta.url));
// The Decentraland contracts on Amoy a trade sends transactions to (contracts.decentraland.org's
// addresses.json under `amoy`), plus the token's own collection.
const AMOY_TRADE_CONTRACTS = [
  AMOY_MANA,
  // The off-chain marketplaces V3, V2 and V1 (MarketplaceV5, V4 and V3 in that list), as the Shop names
  // them in apps/ui3/src/shop/checkout/chain/contracts.ts.
  "0x36fd1434a6c4b8ade80c9847c1d15033ce34488c",
  "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7",
  "0x6ab20ae56673ed65f520b7be332aeb61b3ed727d",
  "0x0c8ad1f6aadf89d2eb19f01a100a6143108fe2b0", // MarketplaceV2: on-chain orders
  "0x4b66eab79cc03a96fb6275cfcdf23c0db431606d", // BidV2
  "0xe36abc9ec616c83caaa386541380829106149d68", // CollectionStore
];
// The primary loop's other destinations: ending a discount calls the CouponManager V3, and a credits
// purchase goes through the CreditsManager (both under `amoy` in addresses.json).
const AMOY_COUPON_MANAGER = "0x6c956587d9fe70032781edcdc626310648575382";
const AMOY_CREDITS_MANAGER = "0x8052a560e6e6ac86eeb7e711a4497f639b322fb3" as const;
const CREDITS_SIGNER_ROLE = keccak256(toHex("CREDITS_SIGNER_ROLE"));
const COLLECTION_ABI = parseAbi(["function creator() view returns (address)", "function name() view returns (string)", "function balanceOf(address) view returns (uint256)"]);
const ROLES_ABI = parseAbi(["function hasRole(bytes32, address) view returns (bool)"]);
// Creator sales follow decentraland.org's `dapps-shop-creator-sales` rollout flag (apps/ui3 discovery/flags.ts),
// which is off; the primary loop turns them on the way a host does, through window.__SHOP_FLAGS__.
const CREATOR_SALES_ON = "window.__SHOP_FLAGS__ = { ...(window.__SHOP_FLAGS__ || {}), creatorSales: true };";

const env = journeyEnv();
const preflightOnly = process.argv.includes("--preflight");
const primaryLoop = process.argv.includes("--primary");
const sites = (env.E2E_SITES_URL ?? env.FOUNDATION_DEV_SITES_URL ?? "").replace(/\/$/, "");
if (!sites) throw new Error("Set E2E_SITES_URL, or run under eval \"$(foundation-dev env)\" so FOUNDATION_DEV_SITES_URL names the stack");
// Trades signed here are written to the stack's database, so it must be a testnet stack's. The profile
// comes from `foundation-dev env` in the shell, never from E2E_ENV_FILE, and only describes the stack
// at FOUNDATION_DEV_SITES_URL, so E2E_SITES_URL may not point anywhere else.
const stackProfile = process.env.FOUNDATION_DEV_STACK_PROFILE;
const stackSites = (process.env.FOUNDATION_DEV_SITES_URL ?? "").replace(/\/$/, "");
if (stackProfile !== "testnet") {
  throw new Error(
    stackProfile
      ? `The stack runs the ${stackProfile} profile; the journey needs a testnet stack (README)`
      : "FOUNDATION_DEV_STACK_PROFILE is not set: run under set -a; eval \"$(foundation-dev env)\"; set +a (README)",
  );
}
if (sites !== stackSites) throw new Error(`E2E_SITES_URL (${sites}) is not the stack foundation-dev env describes (${stackSites})`);
const price = env.E2E_PRICE ?? "1";
const repriced = env.E2E_REPRICE ?? "2";
const bid = env.E2E_BID ?? "1";
const amoy = createPublicClient({ chain: polygonAmoy, transport: http(env.E2E_RPC_AMOY ?? DEFAULT_RPC[polygonAmoy.id]) });

type Actor = { name: string; wallet: TestnetWallet; context: BrowserContext; page: Page };
type Step = { name: string; ok: boolean; ms: number; detail?: string; tx: Hex[] };
const steps: Step[] = [];

async function actor(browser: Browser, name: string, key: Hex, allowedTo: string[], initScript?: string): Promise<Actor> {
  const wallet = testnetWallet({ key, chainId: polygonAmoy.id, allowedTo });
  // No __SHOP_CHAIN__ override: the Shop settles on whatever the profile's VITE_SHOP_CHAIN_ID built it
  // for, and the wallet refuses Polygon, so a stack not wired for Amoy fails at its first signature.
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  // Only the Shop's own top-level page may reach the wallet: no iframe, no other origin.
  await context.exposeBinding("__e2eWallet", (source, req: { method: string; params?: unknown[] }) => {
    if (source.frame !== source.page.mainFrame() || new URL(source.frame.url()).origin !== new URL(sites).origin) {
      return { error: { code: 4100, message: `refused: ${source.frame.url()} is not the Shop's page` } };
    }
    return answer(wallet, req);
  });
  await context.addInitScript(PAGE_PROVIDER);
  if (initScript) await context.addInitScript(initScript);
  const page = await context.newPage();
  wallet.onChainChanged = (id) => void page.evaluate((hex) => (window as never as { __e2eWalletEmit: (e: string, v: string) => void }).__e2eWalletEmit("chainChanged", hex), `0x${id.toString(16)}`).catch(() => {});
  page.on("pageerror", (e) => console.log(`  [${name}] page error: ${e.message}`));
  return { name, wallet, context, page };
}

async function step(name: string, who: Actor, run: () => Promise<string | void>): Promise<void> {
  const started = Date.now();
  const before = who.wallet.sent.length;
  process.stdout.write(`• ${name} … `);
  try {
    const detail = (await run()) ?? undefined;
    steps.push({ name, ok: true, ms: Date.now() - started, detail, tx: who.wallet.sent.slice(before) });
    console.log(`ok (${((Date.now() - started) / 1000).toFixed(1)}s)${detail ? ` ${detail}` : ""}`);
  } catch (e) {
    mkdirSync(OUT, { recursive: true });
    const shot = `${OUT}${name.replace(/\W+/g, "-")}.png`;
    await who.page.screenshot({ path: shot, fullPage: true }).catch(() => {});
    steps.push({ name, ok: false, ms: Date.now() - started, detail: (e as Error).message, tx: who.wallet.sent.slice(before) });
    console.log(`FAILED\n  ${(e as Error).message.split("\n")[0]}\n  screenshot: ${shot}`);
    throw e;
  }
}

async function signIn(a: Actor): Promise<void> {
  // networkidle: the dev server's first visit compiles modules, and a click before hydration does nothing.
  await a.page.goto(`${sites}/shop`, { waitUntil: "networkidle" });
  await a.page.getByRole("button", { name: "Sign in" }).or(a.page.getByRole("link", { name: "Sign in" })).first().click();
  // The Shop's sign-in leaves for the login page, whose injected-wallet option is MetaMask's (PAGE_PROVIDER
  // reports isMetaMask). It is a first option when the browser has a wallet, or under "More options".
  await a.page.waitForURL(/\/auth\/login/, { timeout: 60_000 });
  await a.page.getByTestId("login-page").waitFor({ timeout: 60_000 });
  await a.page.waitForLoadState("networkidle");
  const metamask = a.page.locator('[data-testid="primary-metamask-button"], [data-testid="more-metamask-button"]');
  if (!(await metamask.first().isVisible())) await a.page.getByTestId("show-more-button").click();
  await metamask.first().click();
  // Back on the Shop, signed in: the top bar's sign-in entry gives way to the account's avatar.
  await a.page.waitForURL((url) => url.pathname.startsWith("/shop"), { timeout: 60_000 });
  await a.page.locator(".dtb__signin").waitFor({ state: "detached", timeout: 60_000 });
}

const tokenPath = (t: { contract: string; tokenId: string }) => `${sites}/shop/token/${t.contract}/${t.tokenId}`;

async function confirm(page: Page, amount: string): Promise<void> {
  await page.getByRole("textbox", { name: "Confirm the price" }).fill(amount);
  await page.getByRole("button", { name: "Proceed" }).click();
}

async function list(a: Actor, t: { contract: string; tokenId: string }, amount: string, mode: "new" | "update"): Promise<string> {
  await a.page.goto(`${tokenPath(t)}/sell`);
  await a.page.getByLabel("Price").fill(amount);
  // An update must replace the listing: a second "List for sale" would leave the first signature live.
  await a.page.getByRole("button", { name: mode === "update" ? "Update" : "List for sale", exact: true }).click({ timeout: 60_000 });
  await confirm(a.page, amount);
  const done = a.page.getByTestId("shop-sell-done");
  await done.waitFor({ timeout: RELAYED });
  return (await done.innerText()).split("\n")[0]!;
}

const ownerOf = (t: { contract: string; tokenId: string }) => amoy.readContract({ address: t.contract as Hex, abi: erc721Abi, functionName: "ownerOf", args: [BigInt(t.tokenId)] });

/** The token's owner becomes `who`: polled, since the UI may confirm before the RPC sees the block. */
async function becomesOwner(t: { contract: string; tokenId: string }, who: Hex): Promise<void> {
  const until = Date.now() + 2 * 60_000;
  // A failed read is retried at the next poll; only the deadline fails the step.
  let owner = await ownerOf(t).catch(() => null);
  while (owner?.toLowerCase() !== who.toLowerCase()) {
    if (Date.now() > until) throw new Error(`ownerOf is still ${owner ?? "unreadable"} after 2 minutes`);
    await new Promise((r) => setTimeout(r, 5_000));
    owner = await ownerOf(t).catch(() => null);
  }
}

/** The rows of the token's Bids tab that the buyer made. */
const buyerBids = (page: Page, buyer: Hex) => page.getByTestId("shop-token-bid").filter({ has: page.locator(`a[title="${buyer.toLowerCase()}" i]`) });

/** A cancel the relay carries: the wallet signs, the .zone relayer pays, and nothing is sent from here. */
function relayed(a: Actor, before: number): void {
  const sent = a.wallet.sent.slice(before);
  if (sent.length) throw new Error(`the Shop fell back to a paid transaction (${sent.join(", ")}) instead of the relay`);
}

type Funds = { pol: bigint; mana: bigint };

async function balances(label: string, address: Hex): Promise<Funds> {
  const [pol, mana] = await Promise.all([
    amoy.getBalance({ address }),
    amoy.readContract({ address: AMOY_MANA, abi: erc20Abi, functionName: "balanceOf", args: [address] }),
  ]);
  console.log(`  ${label} ${address}: ${formatEther(pol)} POL, ${formatEther(mana)} MANA (Amoy)`);
  return { pol, mana };
}

/** The trade loop over E2E_TOKEN: listings, bids and a resale, between the two wallets. */
async function trade(seller: Actor, buyer: Actor, s: Funds, b: Funds, token: { contract: string; tokenId: string } | null): Promise<void> {
  if (token) {
    await step("seller owns the token", seller, async () => {
      const owner = await ownerOf(token);
      if (owner.toLowerCase() !== seller.wallet.address.toLowerCase()) throw new Error(`the token belongs to ${owner}, not the seller`);
    });
    await step("token page renders", seller, async () => {
      await seller.page.goto(tokenPath(token));
      await seller.page.getByRole("heading").first().waitFor({ timeout: 60_000 });
      if (await seller.page.getByText(/not found/i).first().isVisible().catch(() => false)) throw new Error("the Shop does not know this token: is it in the Amoy collections subgraph?");
    });
    await step("no bid of the buyer is left open", seller, async () => {
      await seller.page.goto(`${tokenPath(token)}/bids`, { waitUntil: "networkidle" });
      const open = await buyerBids(seller.page, buyer.wallet.address).count();
      if (open) throw new Error(`the buyer already has ${open} open bid(s) on this token, from an earlier run: cancel them first (README)`);
    });
  }
  const short = [
    s.pol === 0n && "seller has no POL (accepting a bid pays gas)",
    b.pol === 0n && "buyer has no POL (gas, should the relay refuse an approval)",
    b.mana < parseEther(bid) && `buyer has less than the ${bid} MANA of the bid`,
    s.mana < parseEther(price) && `seller has less than the ${price} MANA it pays to buy the token back`,
  ].filter(Boolean);
  if (preflightOnly || !token) {
    console.log(`\nPreflight done.${short.length ? `\nBefore the full loop: ${short.join("; ")}.` : ""}`);
    return;
  }
  if (short.length) throw new Error(`not funded: ${short.join("; ")}`);

  // A listing an earlier run left open is the token's order in /v1/nfts: the loop starts from an unlisted token.
  await step("no listing of the seller is left open", seller, async () => {
    type Order = { tradeId?: string | null; owner?: string };
    const tokenOrder = async (): Promise<Order | null> => {
      const res = await fetch(`${sites}/shop-api/v1/nfts?contractAddress=${token.contract}&tokenId=${token.tokenId}`);
      if (!res.ok) {
        await res.body?.cancel();
        throw new Error(`/v1/nfts answered ${res.status}`);
      }
      const body = (await res.json()) as { data?: { order?: Order | null }[] };
      if (!body.data?.length) throw new Error("/v1/nfts does not know the token");
      return body.data[0]!.order ?? null;
    };
    const order = await tokenOrder();
    if (!order) return "none open";
    if (order.owner?.toLowerCase() !== seller.wallet.address.toLowerCase()) throw new Error(`the open listing is ${order.owner}'s, not the seller's`);
    const before = seller.wallet.sent.length;
    await seller.page.goto(`${tokenPath(token)}/cancel`);
    await seller.page.getByRole("button", { name: "Confirm" }).click();
    await seller.page.getByTestId("shop-remove-done").waitFor({ timeout: RELAYED });
    relayed(seller, before);
    // The cancellation is read from the chain: wait until the token shows no order.
    const until = Date.now() + RELAYED;
    for (;;) {
      const again = await tokenOrder().catch(() => order);
      if (!again) break;
      if (Date.now() > until) throw new Error("the cancelled listing is still the token's order");
      await new Promise((r) => setTimeout(r, 5_000));
    }
    return `cancelled ${order.tradeId || "a legacy order"} from an earlier run`;
  });

  // The seller's listing: list, reprice, cancel (cancelSignature, relayed).
  await step(`seller lists for ${price} MANA`, seller, () => list(seller, token, price, "new"));
  await step(`seller changes the price to ${repriced} MANA`, seller, () => list(seller, token, repriced, "update"));
  await step("seller cancels the listing, relayed", seller, async () => {
    const before = seller.wallet.sent.length;
    await seller.page.goto(`${tokenPath(token)}/cancel`);
    await seller.page.getByRole("button", { name: "Confirm" }).click();
    await seller.page.getByTestId("shop-remove-done").waitFor({ timeout: RELAYED });
    relayed(seller, before);
  });

  // The buyer's bid: bid, cancel, bid again; then the seller accepts (a transaction the seller pays for).
  const placeBid = async () => {
    await buyer.page.goto(`${tokenPath(token)}/bid`);
    await buyer.page.getByLabel("Price").fill(bid);
    await buyer.page.getByRole("button", { name: "Bid", exact: true }).click();
    await confirm(buyer.page, bid);
    await buyer.page.getByTestId("shop-bid-done").waitFor({ timeout: RELAYED });
  };
  await step(`buyer bids ${bid} MANA`, buyer, placeBid);
  await step("buyer cancels the bid, relayed", buyer, async () => {
    const before = buyer.wallet.sent.length;
    await buyer.page.goto(`${tokenPath(token)}/bids`);
    await buyerBids(buyer.page, buyer.wallet.address).getByRole("button", { name: "Cancel" }).click();
    await buyer.page.getByText("Bid cancelled.").waitFor({ timeout: RELAYED });
    relayed(buyer, before);
  });
  await step(`buyer bids ${bid} MANA again`, buyer, placeBid);
  await step("seller accepts the bid", seller, async () => {
    await seller.page.goto(`${tokenPath(token)}/bids`);
    // The buyer's bid, not whichever is listed first: a stale or third-party bid would send the token elsewhere.
    await buyerBids(seller.page, buyer.wallet.address).getByRole("button", { name: "Accept" }).click();
    await confirm(seller.page, bid);
    await seller.page.getByTestId("shop-accept-done").waitFor({ timeout: RELAYED });
  });
  await step("the buyer owns the token on chain", buyer, () => becomesOwner(token, buyer.wallet.address));

  // Back to the seller: the buyer lists it, the seller buys it with MANA through the cart.
  await step(`buyer lists it for ${price} MANA`, buyer, () => list(buyer, token, price, "new"));
  await step("seller buys it back with MANA", seller, async () => {
    await seller.page.goto(tokenPath(token));
    const mana = seller.page.getByTestId("pay-mana");
    if (await mana.isVisible().catch(() => false)) await mana.click();
    await seller.page.getByTestId("cta-buy").click();
    await seller.page.waitForURL(/\/shop\/cart/);
    // Either the cart asks to approve MANA first, or it goes straight to the purchase.
    await checkout(seller.page);
  });
  await step("the seller owns the token again", seller, () => becomesOwner(token, seller.wallet.address));
}

/** Confirms the cart and walks its steps (approvals, the credits authorization, the purchase) to the receipt. */
async function checkout(page: Page): Promise<void> {
  await page.getByTestId("confirm-checkout").click();
  // The step's button is disabled while its approval runs (AuthorizeStep's `busy`), and the modal goes away
  // once the cart moves on, so each pass waits for an enabled button, the receipt or the step's error.
  const ready = page.locator('[data-testid="authorize-step-action"]:not([disabled])');
  const busy = page.locator('[data-testid="authorize-step-action"][disabled]');
  // Errors show in the approval modal or, for a refused authorization, a revert or a declined approval, in the group.
  const failed = page.getByTestId("authorize-step").getByRole("alert").or(page.locator(".shop-group__status[role=alert]"));
  const done = page.getByTestId("success-done");
  for (let n = 0; n < 6; n++) {
    await ready.or(done).or(failed).first().waitFor({ timeout: RELAYED });
    if (await done.isVisible()) return;
    // A failed approval re-enables its button: report it rather than clicking it again.
    if (await failed.isVisible()) throw new Error(`the cart's step failed: ${(await failed.first().innerText()).trim()}`);
    await ready.first().click();
    await busy.waitFor({ timeout: 5_000 }).catch(() => {});
  }
  throw new Error("the cart asked for more than 6 steps without reaching the receipt");
}

const itemPath = (i: { contract: string; itemId: string }) => `${sites}/shop/item/${i.contract}/${i.itemId}`;
const unitsOf = (i: { contract: string }, who: Hex) => amoy.readContract({ address: i.contract as Hex, abi: COLLECTION_ABI, functionName: "balanceOf", args: [who] });

/** The buyer's units of the collection grow past `before`: polled, as the receipt may come before the RPC's block. */
async function unitsGrow(i: { contract: string }, who: Hex, before: bigint): Promise<bigint> {
  const until = Date.now() + 2 * 60_000;
  for (;;) {
    const now = await unitsOf(i, who).catch(() => before);
    if (now > before) return now;
    if (Date.now() > until) throw new Error(`the buyer still holds ${before} unit(s) of the collection after 2 minutes`);
    await new Promise((r) => setTimeout(r, 5_000));
  }
}

/**
 * Picks `method` on the item page and waits until the price shown is that method's. E2E_ITEM is listed for
 * both, so a missing choice is an error: with one method the panel renders no choice, and credits (the
 * default for a primary item) would otherwise be bought or read as MANA.
 */
async function selectMethod(page: Page, method: "mana" | "credits"): Promise<void> {
  const choice = page.getByTestId(`pay-${method}`);
  if (!(await choice.waitFor({ timeout: 30_000 }).then(() => true, () => false))) {
    throw new Error(`the item page offers no ${method} choice: E2E_ITEM must be listed in the Shop for both credits and MANA`);
  }
  await choice.click();
  await page.locator(`.shop-pay__pricecol[data-method="${method}"]`).waitFor({ timeout: 30_000 });
}

/** Buys one unit from the item page, paying with `method`, through the cart. */
async function buyItem(a: Actor, i: { contract: string; itemId: string }, method: "mana" | "credits"): Promise<void> {
  // Not networkidle: a signed-in page keeps the credits balance stream open. The panel first, then the
  // method: credits is the default for a primary item, and a buyer short of credits sees a top-up, not Buy.
  await a.page.goto(itemPath(i));
  await a.page.getByTestId("payment-panel").waitFor({ timeout: 60_000 });
  await selectMethod(a.page, method);
  const buy = a.page.getByTestId("cta-buy");
  const topUp = a.page.getByTestId("cta-top-up");
  await buy.or(topUp).first().waitFor({ timeout: 60_000 });
  if (!(await buy.isVisible())) throw new Error(`the item page asks to top up instead of buying with ${method}`);
  await buy.click();
  await a.page.waitForURL(/\/shop\/cart/);
  await checkout(a.page);
}

type CreditsStatus = { authorizeEnabled?: boolean; ready?: boolean; chainId?: number; contract?: string | null };

const manaOf = (who: Hex) => amoy.readContract({ address: AMOY_MANA, abi: erc20Abi, functionName: "balanceOf", args: [who] });

/** The buyer's MANA once it differs from `before`, polled: the RPC may answer from a block behind the receipt. */
async function manaMoves(who: Hex, before: bigint): Promise<bigint> {
  const until = Date.now() + 60_000;
  for (;;) {
    const now = await manaOf(who).catch(() => before);
    if (now !== before || Date.now() > until) return now;
    await new Promise((r) => setTimeout(r, 5_000));
  }
}

/** The MANA amount the item page quotes once MANA is selected, or null when it cannot be read. */
async function quotedMana(page: Page): Promise<bigint | null> {
  await selectMethod(page, "mana");
  const text = await page.getByTestId("item-price").innerText({ timeout: 10_000 }).catch(() => "");
  const n = text.replace(/,/g, "").match(/\d+(?:\.\d+)?/)?.[0];
  try {
    return n ? parseEther(n) : null;
  } catch {
    return null;
  }
}

const MANA_TRANSFER = keccak256(toHex("Transfer(address,address,uint256)"));
// ERC-721 shares the signature; its token id is the third indexed topic.
const ERC721_TRANSFER = MANA_TRANSFER;
const FEED = parseAbi([
  "function manaUsdAggregator() view returns (address)",
  "function latestRoundData() view returns (uint80, int256, uint256, uint256, uint80)",
  "function decimals() view returns (uint8)",
]);

/**
 * The buyer's unit was minted by a transaction that settled through acceptWithCoupon with the seller's rate
 * coupon for `pct`, covering the collection, applied by Amoy's coupon manager, and charging the MANA the contract
 * charges: the coupon on each signed amount, then a USD price converted at the feed round of that block.
 */
const AMOY_MARKETPLACE_V3 = "0x36fd1434a6c4b8ade80c9847c1d15033ce34488c";
const COLLECTION_V2 = parseAbi([
  "event Transfer(address indexed from, address indexed to, uint256 indexed tokenId)",
  "function decodeTokenId(uint256 id) view returns (uint256 itemId, uint256 issuedId)",
]);

/** Transaction hashes the page sent while `run` ran: the wallet's own sends and the relay's `txHash` answers. */
async function hashesSentDuring(a: Actor, run: () => Promise<void>): Promise<Hex[]> {
  const relayed: Hex[] = [];
  const pending: Promise<void>[] = [];
  const listener = (response: Response) => {
    if (response.request().method() !== "POST") return;
    pending.push(
      response.json().then(
        (body: unknown) => {
          const hash = (body as { txHash?: unknown } | null)?.txHash;
          if (typeof hash === "string" && /^0x[0-9a-fA-F]{64}$/.test(hash)) relayed.push(hash as Hex);
        },
        () => {},
      ),
    );
  };
  const before = a.wallet.sent.length;
  a.page.on("response", listener);
  try {
    await run();
  } finally {
    a.page.off("response", listener);
    await Promise.all(pending);
  }
  return [...new Set([...a.wallet.sent.slice(before), ...relayed].map((h) => h.toLowerCase() as Hex))];
}

/**
 * The transaction that minted one unit of `item` to the buyer: among the hashes the purchase sent, the one
 * whose receipt mints it, or, when the page sent none we could see, the single such mint since `fromBlock`.
 * A mint is a Transfer from the zero address to the buyer of a token whose item is `item.itemId`.
 */
async function purchaseMint(item: { contract: string; itemId: string }, buyer: Hex, sent: Hex[], fromBlock: bigint): Promise<Hex> {
  const ofItem = async (tokenId: bigint) => {
    const [itemId] = await amoy.readContract({ address: item.contract as Hex, abi: COLLECTION_V2, functionName: "decodeTokenId", args: [tokenId] });
    return itemId === BigInt(item.itemId);
  };
  const mints = async (logs: { address: string; topics: readonly Hex[] }[]) => {
    const found: bigint[] = [];
    for (const l of logs) {
      if (l.address.toLowerCase() !== item.contract.toLowerCase() || l.topics.length !== 4 || l.topics[0] !== ERC721_TRANSFER) continue;
      if (BigInt(l.topics[1]!) !== 0n || BigInt(l.topics[2]!) !== BigInt(buyer)) continue;
      if (await ofItem(BigInt(l.topics[3]!))) found.push(BigInt(l.topics[3]!));
    }
    return found;
  };
  if (sent.length) {
    const minting: Hex[] = [];
    for (const hash of sent) {
      const receipt = await amoy.getTransactionReceipt({ hash }).catch(() => null);
      if (receipt && (await mints(receipt.logs)).length === 1) minting.push(hash);
    }
    if (minting.length === 1) return minting[0]!;
    throw new Error(`of the ${sent.length} transaction(s) the purchase sent (${sent.join(", ")}), ${minting.length} minted item ${item.itemId} to the buyer, not one`);
  }
  const logs = await amoy.getLogs({ address: item.contract as Hex, event: COLLECTION_V2[0], args: { from: zeroAddress, to: buyer }, fromBlock });
  const hashes = new Set<Hex>();
  for (const l of logs) if (l.transactionHash && (await ofItem(l.args.tokenId!))) hashes.add(l.transactionHash);
  if (hashes.size === 1) return [...hashes][0]!;
  throw new Error(`the page reported no transaction hash, and ${hashes.size} transactions minted item ${item.itemId} to the buyer since the purchase started, not one`);
}

async function couponPurchase(item: { contract: string; itemId: string }, buyer: Hex, seller: Hex, pct: number, fromBlock: bigint, sent: Hex[]): Promise<string> {
  const hash = await purchaseMint(item, buyer, sent, fromBlock);
  const [tx, receipt] = await Promise.all([amoy.getTransaction({ hash }), amoy.getTransactionReceipt({ hash })]);
  if (receipt.status !== "success") throw new Error(`${hash} reverted`);
  if (tx.to?.toLowerCase() !== AMOY_MARKETPLACE_V3) throw new Error(`${hash} called ${tx.to}, not the off-chain marketplace V3 the coupon manager is paired with`);
  const settled = settledCall(tx.input);
  if (settled.call !== "acceptWithCoupon") throw new Error(`${hash} settled through ${settled.call}: the discount was not applied on chain`);
  // The trade that sells this item, and the coupon and proof at its positions: coupon i applies to trade i,
  // proof j to sent asset j.
  const at = soldItem(settled, item.contract as Hex, BigInt(item.itemId));
  const trade = settled.trades[at.trade]!;
  const coupon = settled.coupons[at.trade];
  if (!coupon) throw new Error(`${hash} carries no coupon for the trade that sells the item`);
  if (trade.signer.toLowerCase() !== seller.toLowerCase()) throw new Error(`${hash} accepted a trade signed by ${trade.signer}, not the seller`);
  const data = couponData(coupon.data);
  if (data.discountType !== 1n || data.discount !== BigInt(pct) * 10_000n) throw new Error(`${hash} carried a coupon of type ${data.discountType} and ${data.discount} ppm, not the ${pct}% rate`);
  if (!collectionInRoot(item.contract as Hex, couponProofs(coupon.callerData)[at.asset] ?? [], data.root)) throw new Error(`${hash} carried no proof that the coupon's root covers the collection`);
  if (!receipt.logs.some((l) => l.address.toLowerCase() === AMOY_COUPON_MANAGER)) throw new Error(`${hash} has no event of Amoy's coupon manager: the coupon was not applied`);
  let round: { answer: bigint; decimals: number } | null = null;
  if (trade.received.some((a) => a.assetType === 2n)) {
    // Read at the receipt's block, which is the state at the END of that block, not at the transaction's
    // position in it. That is exact for Amoy's mock aggregator, whose answer is constant (only updatedAt
    // follows block.timestamp); against a live feed updated in the same block it could differ.
    const atBlock = { blockNumber: receipt.blockNumber };
    const aggregator = await amoy.readContract({ address: tx.to!, abi: FEED, functionName: "manaUsdAggregator", ...atBlock });
    const [[, answer], decimals] = await Promise.all([
      amoy.readContract({ address: aggregator, abi: FEED, functionName: "latestRoundData", ...atBlock }),
      amoy.readContract({ address: aggregator, abi: FEED, functionName: "decimals", ...atBlock }),
    ]);
    round = { answer, decimals };
  }
  const expected = chargedMana(trade.received, data.discount, round);
  const paid = receipt.logs
    // Transfer events only: the same MANA log stream carries the buyer's Approval, whose value is an allowance.
    .filter((l) => l.address.toLowerCase() === AMOY_MANA && l.topics[0] === MANA_TRANSFER && l.topics[1] && BigInt(l.topics[1]) === BigInt(buyer))
    .reduce((sum, l) => sum + BigInt(l.data), 0n);
  if (paid !== expected) throw new Error(`${hash} took ${formatEther(paid)} MANA from the buyer; the contract charges ${formatEther(expected)} for this coupon`);
  return `acceptWithCoupon at ${pct}% in ${hash}${sent.length ? "" : " (found by its mint)"}: ${formatEther(paid)} MANA, as the contract charges`;
}

/** The seller's live discounts on this collection in My Store: rows that still offer "End now". */
/** "<pct>% off" at the start of a row: "5% off" must not match "15% off". */
const offText = (pct: number) => new RegExp(`^\\s*${pct}% off`);

/** My Store's Discounts list once it has answered: the empty state or its rows, never the loading state. */
async function discountsLoaded(page: Page): Promise<void> {
  const panel = page.locator('section[aria-label="Discounts"]');
  await panel.getByTestId("shop-empty").or(panel.getByTestId("shop-creator-sale")).or(panel.getByRole("alert")).first().waitFor({ timeout: 60_000 });
  if (await panel.getByRole("alert").first().isVisible().catch(() => false)) throw new Error(`My Store could not load the discounts: ${(await panel.getByRole("alert").first().innerText()).trim()}`);
}

/** The "Discount <name>" button for the collection, or an error naming the buttons My Store does show. */
async function discountButton(page: Page, collectionName: string) {
  const button = page.getByRole("button", { name: `Discount ${collectionName}`, exact: true });
  // The buttons come from the catalogue and sale-state queries, which load after the discounts list.
  if (await button.waitFor({ timeout: 60_000 }).then(() => true, () => false)) return button;
  const shown = (await page.getByRole("button", { name: /^Discount / }).allInnerTexts()).map((t) => t.trim());
  throw new Error(`My Store has no “Discount ${collectionName}” button (the collection's on-chain name)${shown.length ? `; it shows: ${shown.join(", ")}` : ", and no collection to discount"}`);
}

/** The seller's live discounts covering the collection, by its address (the row's label may still be the address). */
function liveSales(page: Page, collection: string) {
  return page.locator(`[data-testid="shop-creator-sale"][data-collections*="${collection.toLowerCase()}"]:is([data-status="active"], [data-status="scheduled"])`);
}

/**
 * Ends this run's discount: the collection's live one at `pct` (the preflight refuses a collection with one
 * live already). `onClicked` hears the moment the transaction is requested, so a retry never sends a second.
 */
async function endDiscount(seller: Actor, collection: string, pct: number, onClicked: () => void): Promise<void> {
  await seller.page.goto(`${sites}/shop/my-store`);
  await discountsLoaded(seller.page);
  const row = liveSales(seller.page, collection).filter({ hasText: offText(pct) });
  await row.first().getByRole("button", { name: "End now" }).click({ timeout: 60_000 });
  onClicked();
  // The row re-renders as ended once the list refreshes, so the outcome is read from the page: the
  // confirmation, or the error its status shows.
  const done = seller.page.getByText("Ended.").first();
  const failed = seller.page.getByTestId("shop-creator-sale").getByRole("alert").first();
  await done.or(failed).waitFor({ timeout: RELAYED });
  if (await failed.isVisible()) throw new Error(`ending the discount failed: ${(await failed.innerText()).trim()}`);
}

/** The primary loop over E2E_ITEM: the seller's discount, then a MANA and a credits purchase at the lower price. */
async function primary(seller: Actor, buyer: Actor, buyerKey: Hex, s: Funds, b: Funds, item: { contract: string; itemId: string } | null): Promise<void> {
  const pct = parseDiscount(env.E2E_DISCOUNT);
  const topup = parseTopup(env.E2E_TOPUP);
  let collectionName = "";
  let price: bigint | null = null;
  if (item) {
    await step("seller created the item's collection", seller, async () => {
      const [creator, name] = await Promise.all([
        amoy.readContract({ address: item.contract as Hex, abi: COLLECTION_ABI, functionName: "creator" }),
        amoy.readContract({ address: item.contract as Hex, abi: COLLECTION_ABI, functionName: "name" }),
      ]);
      if (creator.toLowerCase() !== seller.wallet.address.toLowerCase()) throw new Error(`the collection's creator is ${creator}, not the seller: only its creator can discount it`);
      collectionName = name;
      return `“${name}”`;
    });
    await step("item page renders", buyer, async () => {
      // The panel, not Buy: credits is the default method, and a buyer without credits sees a top-up.
      await buyer.page.goto(itemPath(item));
      await buyer.page.getByTestId("payment-panel").waitFor({ timeout: 60_000 });
      // Listed for both methods, and its MANA price readable: the funding and discount checks rest on it.
      await selectMethod(buyer.page, "credits");
      price = await quotedMana(buyer.page);
      if (price === null) throw new Error("the item page's MANA price could not be read");
      return `${formatEther(price)} MANA before the discount`;
    });
    await step("no discount of the seller's is live on the collection", seller, async () => {
      await seller.page.goto(`${sites}/shop/my-store`);
      // Counted only once the list has answered: while it loads it shows no row, live or not.
      await discountsLoaded(seller.page);
      const live = await liveSales(seller.page, item.contract).count();
      if (live) throw new Error(`${live} discount(s) on “${collectionName}” are still live, from an earlier run: end them in My Store first`);
      await discountButton(seller.page, collectionName);
    });
  }
  // Credits: the stack signs them only with a key holding CREDITS_SIGNER_ROLE on Amoy's CreditsManager,
  // and only once its usage index has caught up (README, "Credits").
  let creditsProblem: string | null = null;
  await step("stack can sign credits on Amoy", buyer, async () => {
    const res = await fetch(`${sites}/credits/usage-index/status`);
    if (!res.ok) {
      await res.body?.cancel();
      throw new Error(`${sites}/credits/usage-index/status answered ${res.status}`);
    }
    const st = (await res.json()) as CreditsStatus;
    const signer = env.E2E_CREDITS_SIGNER;
    if (st.contract?.toLowerCase() !== AMOY_CREDITS_MANAGER || st.chainId !== 80002) creditsProblem = `the stack's credits contract is ${st.contract ?? "unset"} on ${st.chainId}, not Amoy's CreditsManager`;
    else if (!st.authorizeEnabled) creditsProblem = "the stack signs no credits: put its signer key in .cache/dev/credits-signer.key and restart it";
    else if (!st.ready) creditsProblem = "the stack's credits usage index has not caught up yet";
    else if (signer && !isAddress(signer)) creditsProblem = "E2E_CREDITS_SIGNER is not an address";
    else if (signer && !(await amoy.readContract({ address: AMOY_CREDITS_MANAGER, abi: ROLES_ABI, functionName: "hasRole", args: [CREDITS_SIGNER_ROLE, signer as Hex] }))) {
      creditsProblem = `${signer} does not hold CREDITS_SIGNER_ROLE on Amoy's CreditsManager`;
    }
    if (creditsProblem && !preflightOnly) throw new Error(creditsProblem);
    return creditsProblem ? `not yet: ${creditsProblem}` : "ready";
  });
  // The discount lowers the MANA price, so the regular price is enough for the MANA purchase.
  const short = [
    s.pol === 0n && "seller has no POL (ending the discount pays gas)",
    b.pol === 0n && "buyer has no POL (gas for the purchases)",
    price !== null ? b.mana < price && `buyer has less than the ${formatEther(price)} MANA the item costs` : b.mana === 0n && "buyer has no MANA for the MANA purchase",
  ].filter(Boolean);
  if (preflightOnly || !item) {
    console.log(`\nPreflight done.${short.length ? `\nBefore the primary loop: ${short.join("; ")}.` : ""}`);
    return;
  }
  if (short.length) throw new Error(`not funded: ${short.join("; ")}`);

  // `started`: the discount may be published (its signature was asked for). `endRequested`: the end
  // transaction was asked for. The cleanup runs only between the two, so it never ends a discount that was
  // not published, nor pays for a second end transaction.
  let started = false;
  let endRequested = false;
  try {
    // Inside the try: a start that times out may still have published the discount, and the preflight
    // made sure no other discount is live on the collection, so whatever the cleanup finds is this run's.
    await step(`seller starts a ${pct}% discount from My Store`, seller, async () => {
      await seller.page.goto(`${sites}/shop/my-store`);
      await discountsLoaded(seller.page);
      await (await discountButton(seller.page, collectionName)).click();
      await seller.page.getByLabel("Discount percentage").fill(String(pct));
      await seller.page.getByRole("button", { name: "Review discount" }).click();
      await seller.page.getByRole("button", { name: "Start discount" }).click();
      started = true;
      await seller.page.getByTestId("shop-sale-started").waitFor({ timeout: RELAYED });
    });
    await step("the item shows the discounted price", buyer, async () => {
      // The catalogue joins the creator's live coupon once the coupon-state job has read it from the manager.
      const until = Date.now() + 3 * 60_000;
      for (;;) {
        await buyer.page.goto(itemPath(item));
        await buyer.page.getByTestId("payment-panel").waitFor({ timeout: 60_000 }).catch(() => {});
        // The server render has no __SHOP_FLAGS__, so the crossed-out price arrives with the client's refetch.
        if (await buyer.page.getByTestId("detail-price-was").waitFor({ timeout: 15_000 }).then(() => true, () => false)) return (await buyer.page.getByTestId("item-price").innerText().catch(() => "")).replace(/\s+/g, " ");
        if (Date.now() > until) throw new Error("no crossed-out regular price after 3 minutes: the discount does not reach the item page");
        await new Promise((r) => setTimeout(r, 15_000));
      }
    });
    await step("buyer buys one with MANA at the discount", buyer, async () => {
      const units = await unitsOf(item, buyer.wallet.address);
      const fromBlock = await amoy.getBlockNumber();
      const sent = await hashesSentDuring(buyer, () => buyItem(buyer, item, "mana"));
      await unitsGrow(item, buyer.wallet.address, units);
      // Not "paid less than the page's price": the page quotes at the node's spot rate and Amoy settles at its own
      // feed, so a lower amount proves nothing. The transaction itself must carry the coupon and charge what the
      // contract charges for it.
      return couponPurchase(item, buyer.wallet.address, seller.wallet.address, pct, fromBlock, sent);
    });
    await step(`buyer tops up ${topup} test credits with the mock card`, buyer, async () => {
      const out = await signedPost<{ available?: string }>(buyerKey, `${sites}/credits/topup/mock-card`, { credits: topup });
      return `${out.available ?? "?"} credits available`;
    });
    await step("buyer buys one with credits at the discount", buyer, async () => {
      const units = await unitsOf(item, buyer.wallet.address);
      const before = await manaOf(buyer.wallet.address);
      await buyItem(buyer, item, "credits");
      await unitsGrow(item, buyer.wallet.address, units);
      // Paid in credits: a fallback to MANA would also deliver the unit, so the MANA balance must not move.
      const after = await manaMoves(buyer.wallet.address, before);
      if (after !== before) throw new Error(`the buyer's MANA went from ${formatEther(before)} to ${formatEther(after)}: the purchase was not paid in credits`);
    });
    await step("seller ends the discount", seller, () => endDiscount(seller, item.contract, pct, () => (endRequested = true)));
  } finally {
    // A run that failed after starting the discount still ends it, so the collection is not left on sale.
    if (started && !endRequested) {
      console.log("  ending the discount this run started …");
      await endDiscount(seller, item.contract, pct, () => (endRequested = true)).then(
        () => console.log("  ended"),
        (e: Error) => console.log(`  could not end it (${e.message.split("\n")[0]}): end it in My Store`),
      );
    }
  }
}

async function main(): Promise<void> {
  const sellerKey = preflightOnly && !env.E2E_SELLER_KEY ? generatePrivateKey() : keyFrom(env, "E2E_SELLER_KEY");
  const buyerKey = preflightOnly && !env.E2E_BUYER_KEY ? generatePrivateKey() : keyFrom(env, "E2E_BUYER_KEY");
  const token = primaryLoop || (preflightOnly && !env.E2E_TOKEN) ? null : parseToken(env.E2E_TOKEN);
  const item = !primaryLoop || (preflightOnly && !env.E2E_ITEM) ? null : parseItem(env.E2E_ITEM);

  // Each actor may send only to what its steps need. The trade loop's wallets trade the token both ways.
  // In the primary loop the seller only ends the discount (CouponManager) and the buyer only pays: MANA
  // approvals, the marketplaces and collection store, and the CreditsManager. Neither calls the collection.
  const tradeTo = [...AMOY_TRADE_CONTRACTS, ...(token ? [token.contract] : [])];
  const sellerTo = primaryLoop ? [AMOY_COUPON_MANAGER] : tradeTo;
  const buyerTo = primaryLoop ? [...AMOY_TRADE_CONTRACTS, AMOY_CREDITS_MANAGER] : tradeTo;
  const flags = primaryLoop ? CREATOR_SALES_ON : undefined;
  const browser = await launch();
  try {
    const seller = await actor(browser, "seller", sellerKey, sellerTo, flags);
    const buyer = await actor(browser, "buyer", buyerKey, buyerTo, flags);
    console.log(`Stack ${sites}, settling on Amoy`);
    const s = await balances("seller", seller.wallet.address);
    const b = await balances("buyer ", buyer.wallet.address);

    await step("stack answers", seller, async () => {
      const res = await fetch(`${sites}/shop-api/v1/trades-mirror`).catch(() => null);
      if (res && !res.ok) await res.body?.cancel();
      if (!res?.ok) throw new Error(`${sites}/shop-api/v1/trades-mirror answered ${res?.status ?? "nothing"}: is foundation-dev up in the testnet profile?`);
      const body = (await res.json()) as { data?: { completedAt?: string | null } };
      return body.data?.completedAt ? `trades mirrored ${body.data.completedAt}` : "trades mirror has no complete pass yet";
    });
    await step("seller signs in", seller, () => signIn(seller));
    await step("buyer signs in", buyer, () => signIn(buyer));
    if (primaryLoop) await primary(seller, buyer, buyerKey, s, b, item);
    else await trade(seller, buyer, s, b, token);
  } finally {
    await browser.close();
    const failed = steps.filter((x) => !x.ok);
    console.log(`\n${steps.length - failed.length}/${steps.length} steps passed`);
    for (const x of steps) for (const h of x.tx) console.log(`  ${x.name}: https://amoy.polygonscan.com/tx/${h}`);
  }
}

main().catch((e) => {
  // A failed step has already said why; anything else (settings, launch, RPC, funding) says it here.
  if (!steps.some((s) => !s.ok)) console.error(`\n${(e as Error).message}`);
  process.exitCode = 1;
});

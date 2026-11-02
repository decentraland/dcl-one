// The Shop's trade loop, end to end on Decentraland's testnet: two real wallets drive the real UI of a
// local stack in the `testnet` profile, and every signature and transaction lands on Amoy.
//
//   node journey.mts --preflight   the stack, sign-in and the wallets' balances; signs nothing on chain
//   node journey.mts               the whole loop (README: what each step costs and who pays)
//
// A complete run leaves the token where it started, so it can run again: the seller lists, reprices and
// cancels; the buyer bids, cancels and bids again; the seller accepts (the buyer now owns the token);
// the buyer lists it and the seller buys it back with MANA through the cart.
import { mkdirSync } from "node:fs";
import { createPublicClient, erc721Abi, erc20Abi, formatEther, http, parseEther, type Hex } from "viem";
import { generatePrivateKey } from "viem/accounts";
import { polygonAmoy } from "viem/chains";
import type { Browser, BrowserContext, Page } from "playwright-core";
import { launch } from "./lib/browser.mts";
import { journeyEnv, parseToken } from "./lib/env.mts";
import { answer, DEFAULT_RPC, keyFrom, PAGE_PROVIDER, testnetWallet, type TestnetWallet } from "./lib/wallet.mts";

const AMOY_MANA = "0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0" as const;
const RELAYED = 4 * 60_000; // a relayed or mined step: signature, relay, inclusion, the Shop's indexing
const OUT = new URL("../../state/shop-trade-journey/", import.meta.url).pathname;

const env = journeyEnv();
const preflightOnly = process.argv.includes("--preflight");
const sites = (env.E2E_SITES_URL ?? env.FOUNDATION_DEV_SITES_URL ?? "").replace(/\/$/, "");
if (!sites) throw new Error("Set E2E_SITES_URL, or run under eval \"$(foundation-dev env)\" so FOUNDATION_DEV_SITES_URL names the stack");
if (env.FOUNDATION_DEV_STACK_PROFILE && env.FOUNDATION_DEV_STACK_PROFILE !== "testnet") {
  throw new Error(`The stack runs the ${env.FOUNDATION_DEV_STACK_PROFILE} profile; the journey needs a testnet stack (README)`);
}
const price = env.E2E_PRICE ?? "1";
const repriced = env.E2E_REPRICE ?? "2";
const bid = env.E2E_BID ?? "1";
const amoy = createPublicClient({ chain: polygonAmoy, transport: http(env.E2E_RPC_AMOY ?? DEFAULT_RPC[polygonAmoy.id]) });

type Actor = { name: string; wallet: TestnetWallet; context: BrowserContext; page: Page };
type Step = { name: string; ok: boolean; ms: number; detail?: string; tx: Hex[] };
const steps: Step[] = [];

async function actor(browser: Browser, name: string, key: Hex): Promise<Actor> {
  const wallet = testnetWallet({ key, chainId: polygonAmoy.id });
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await context.addInitScript(`window.__SHOP_CHAIN__ = { settlementChainId: ${polygonAmoy.id} };`);
  // Only the Shop's own top-level page may reach the wallet: no iframe, no other origin.
  await context.exposeBinding("__e2eWallet", (source, req: { method: string; params?: unknown[] }) => {
    if (source.frame !== source.page.mainFrame() || new URL(source.frame.url()).origin !== new URL(sites).origin) {
      return { error: { code: 4100, message: `refused: ${source.frame.url()} is not the Shop's page` } };
    }
    return answer(wallet, req);
  });
  await context.addInitScript(PAGE_PROVIDER);
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
  await a.page.getByText("Continue with wallet").click();
  await a.page.getByRole("button", { name: /Continue with E2E testnet wallet/ }).click();
  // Signed in: the top bar's sign-in entry gives way to the account's avatar.
  await a.page.locator(".dtb__signin").waitFor({ state: "detached", timeout: 60_000 });
}

const tokenPath = (t: { contract: string; tokenId: string }) => `${sites}/shop/token/${t.contract}/${t.tokenId}`;

async function confirm(page: Page, amount: string): Promise<void> {
  await page.getByRole("textbox", { name: "Confirm the price" }).fill(amount);
  await page.getByRole("button", { name: "Proceed" }).click();
}

async function list(a: Actor, t: { contract: string; tokenId: string }, amount: string): Promise<string> {
  await a.page.goto(`${tokenPath(t)}/sell`);
  await a.page.getByLabel("Price").fill(amount);
  await a.page.getByRole("button", { name: /^(List for sale|Update)$/ }).click();
  await confirm(a.page, amount);
  const done = a.page.getByTestId("shop-sell-done");
  await done.waitFor({ timeout: RELAYED });
  return (await done.innerText()).split("\n")[0]!;
}

const ownerOf = (t: { contract: string; tokenId: string }) => amoy.readContract({ address: t.contract as Hex, abi: erc721Abi, functionName: "ownerOf", args: [BigInt(t.tokenId)] });

/** The token's owner becomes `who`: polled, since the UI may confirm before the RPC sees the block. */
async function becomesOwner(t: { contract: string; tokenId: string }, who: Hex): Promise<void> {
  const until = Date.now() + 2 * 60_000;
  let owner = await ownerOf(t);
  while (owner.toLowerCase() !== who.toLowerCase()) {
    if (Date.now() > until) throw new Error(`ownerOf is still ${owner} after 2 minutes`);
    await new Promise((r) => setTimeout(r, 5_000));
    owner = await ownerOf(t);
  }
}

async function balances(label: string, address: Hex): Promise<{ pol: bigint; mana: bigint }> {
  const [pol, mana] = await Promise.all([
    amoy.getBalance({ address }),
    amoy.readContract({ address: AMOY_MANA, abi: erc20Abi, functionName: "balanceOf", args: [address] }),
  ]);
  console.log(`  ${label} ${address}: ${formatEther(pol)} POL, ${formatEther(mana)} MANA (Amoy)`);
  return { pol, mana };
}

async function main(): Promise<void> {
  const sellerKey = preflightOnly && !env.E2E_SELLER_KEY ? generatePrivateKey() : keyFrom(env, "E2E_SELLER_KEY");
  const buyerKey = preflightOnly && !env.E2E_BUYER_KEY ? generatePrivateKey() : keyFrom(env, "E2E_BUYER_KEY");
  const token = preflightOnly && !env.E2E_TOKEN ? null : parseToken(env.E2E_TOKEN);

  const browser = await launch();
  try {
    const seller = await actor(browser, "seller", sellerKey);
    const buyer = await actor(browser, "buyer", buyerKey);
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
    if (token) {
      await step("seller owns the token", seller, async () => {
        const owner = await ownerOf(token);
        if (owner.toLowerCase() !== seller.wallet.address.toLowerCase()) throw new Error(`the token belongs to ${owner}, not the seller`);
      });
      await step("token page renders", seller, async () => {
        await seller.page.goto(tokenPath(token));
        await seller.page.getByRole("heading").first().waitFor({ timeout: 60_000 });
        if (await seller.page.getByText(/not found/i).first().isVisible().catch(() => false)) throw new Error("the Shop does not know this token: has the NFT index caught up?");
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

    // The seller's listing: list, reprice, cancel (cancelSignature, relayed).
    await step(`seller lists for ${price} MANA`, seller, () => list(seller, token, price));
    await step(`seller changes the price to ${repriced} MANA`, seller, () => list(seller, token, repriced));
    await step("seller cancels the listing", seller, async () => {
      await seller.page.goto(`${tokenPath(token)}/cancel`);
      await seller.page.getByRole("button", { name: "Confirm" }).click();
      await seller.page.getByTestId("shop-remove-done").waitFor({ timeout: RELAYED });
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
    await step("buyer cancels the bid", buyer, async () => {
      await buyer.page.goto(`${tokenPath(token)}/bids`);
      await buyer.page.getByTestId("shop-token-bid").getByRole("button", { name: "Cancel" }).first().click();
      await buyer.page.getByText("Bid cancelled.").waitFor({ timeout: RELAYED });
    });
    await step(`buyer bids ${bid} MANA again`, buyer, placeBid);
    await step("seller accepts the bid", seller, async () => {
      await seller.page.goto(`${tokenPath(token)}/bids`);
      // The buyer's bid, not whichever is listed first: a stale or third-party bid would send the token elsewhere.
      const row = seller.page.getByTestId("shop-token-bid").filter({ has: seller.page.locator(`a[title="${buyer.wallet.address.toLowerCase()}" i]`) });
      await row.getByRole("button", { name: "Accept" }).click();
      await confirm(seller.page, bid);
      await seller.page.getByTestId("shop-accept-done").waitFor({ timeout: RELAYED });
    });
    await step("the buyer owns the token on chain", buyer, () => becomesOwner(token, buyer.wallet.address));

    // Back to the seller: the buyer lists it, the seller buys it with MANA through the cart.
    await step(`buyer lists it for ${price} MANA`, buyer, () => list(buyer, token, price));
    await step("seller buys it back with MANA", seller, async () => {
      await seller.page.goto(tokenPath(token));
      const mana = seller.page.getByTestId("pay-mana");
      if (await mana.isVisible().catch(() => false)) await mana.click();
      await seller.page.getByTestId("cta-buy").click();
      await seller.page.waitForURL(/\/shop\/cart/);
      await seller.page.getByTestId("confirm-checkout").click();
      // Either the cart asks to approve MANA first, or it goes straight to the purchase.
      const approve = seller.page.getByTestId("authorize-step-action");
      const done = seller.page.getByTestId("success-done");
      await approve.or(done).first().waitFor({ timeout: RELAYED });
      if (await approve.isVisible()) {
        await approve.click();
        await done.waitFor({ timeout: RELAYED });
      }
    });
    await step("the seller owns the token again", seller, () => becomesOwner(token, seller.wallet.address));
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

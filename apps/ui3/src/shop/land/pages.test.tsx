import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";

import { erc20Abi } from "./evm";
import { estateRegistryAbi } from "./estate";
import { estateResult, nameResult, NET, OWNER, parcelResult, rentalListing, sampleTiles, saleOrder, TENANT } from "./fixtures";
import LandsPage from "./LandsPage";
import ManageAsset from "./ManageAsset";
import RentPage from "./RentPage";
import { rentalsAbi } from "./rentals";
import { fakeWallet, stubClient, type Answer } from "./testkit";
import { ShopHarness, type SessionKind } from "./testkit-react";
import { MapNotReadyError } from "./tiles";
import { erc721Abi } from "./evm";

const MANA = 10n ** 18n;
const timing = { poll: { retries: 2, delayMs: 0 }, wait: { pollMs: 0, timeoutMs: 50 } };
const tiles = async () => ({ tiles: sampleTiles(), lastModified: null });
const nfts = (results: unknown[], status = 200): Answer => ({ status, body: status === 200 ? { data: results, total: results.length } : { error: "Service Unavailable", message: "nft-indexer is catching up", source: "nft-indexer" } });

function readerOf(wallet: ReturnType<typeof fakeWallet>) {
  return { chainId: 1, call: (to: string, data: Hex) => wallet.request({ method: "eth_call", params: [{ to, data }] }) as Promise<Hex> };
}

function noCredits() {
  expect(document.body.textContent ?? "").not.toMatch(/credit/i);
}

async function enabledButton(name: string) {
  await waitFor(() => expect(screen.getByRole("button", { name })).toBeEnabled());
  return screen.getByRole("button", { name });
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("/shop/lands", () => {
  const nftReads = (client: { requests: { url: string }[] }) => client.requests.map((r) => new URL(r.url)).filter((u) => u.pathname === "/shop-api/v1/nfts").map((u) => u.searchParams);
  const renderLands = (session: SessionKind, path: string, routes: Record<string, Answer>, loadTiles = tiles) => {
    const client = stubClient(routes);
    render(
      <ShopHarness session={session} path={path} route="/shop/lands">
        <LandsPage client={client} loadTiles={loadTiles} />
      </ShopHarness>,
    );
    return client;
  };

  it("lists LAND on sale with MANA prices and rent per day, never a credit price", async () => {
    const onSale = parcelResult({ order: saleOrder((4000n * MANA).toString()) });
    const forRent = estateResult({ rental: rentalListing({ contractAddress: NET.estate }) });
    const client = renderLands("web3", "/shop/lands?onlyOnSale=true", { "GET /shop-api/v1/nfts": nfts([onSale, forRent]) });
    expect(await screen.findByRole("link", { name: /Parcel 12,-4/ })).toHaveAttribute("href", `/shop/token/${NET.land}/${onSale.nft.tokenId}`);
    expect(screen.getByText("4K")).toBeInTheDocument();
    expect(screen.getByText("/day")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Corner Estate, 3 parcels/ })).toBeInTheDocument();
    expect(nftReads(client)).toHaveLength(1);
    const browse = nftReads(client)[0]!;
    expect([browse.get("isLand"), browse.get("isOnSale"), browse.get("sortBy"), browse.get("owner")]).toEqual(["true", "true", "recently_listed", null]);
    noCredits();
  });

  it("opens on all LAND, newest first, as the card grid (upstream's /lands default)", async () => {
    const client = renderLands("web3", "/shop/lands", { "GET /shop-api/v1/nfts": nfts([]) });
    await screen.findByText("No LAND matches these filters");
    const browse = nftReads(client)[0]!;
    expect([browse.get("isLand"), browse.get("isOnSale"), browse.get("isOnRent"), browse.get("sortBy")]).toEqual(["true", null, null, "newest"]);
    expect(screen.getByRole("radio", { name: "All land" })).toBeChecked();
  });

  it("filters for rent through the URL and asks the index for rentals", async () => {
    const client = renderLands("web3", "/shop/lands?isMap=false", { "GET /shop-api/v1/nfts": nfts([]) });
    const browse = () => nftReads(client).filter((q) => !q.has("owner"));
    await screen.findByText("No LAND matches these filters");
    fireEvent.click(screen.getByRole("radio", { name: "Available to rent" }));
    await waitFor(() => expect(browse().length).toBe(2));
    const q = browse()[1]!;
    expect([q.get("isOnRent"), q.get("sortBy"), q.get("isOnSale")]).toEqual(["true", "rental_listing_date", null]);
    fireEvent.click(await screen.findByRole("checkbox", { name: "30 days" }));
    await waitFor(() => expect(browse().at(-1)!.getAll("rentalDays")).toEqual(["30"]));
  });

  it("leaves dissolved (0 LAND) estates out like upstream, unless a size filter is set", async () => {
    const client = renderLands("web3", "/shop/lands", { "GET /shop-api/v1/nfts": nfts([]) });
    await screen.findByText("No LAND matches these filters");
    expect(nftReads(client)[0]!.get("minEstateSize")).toBe("1");
    fireEvent.click(screen.getByRole("button", { name: "Parcels" }));
    await waitFor(() => expect(nftReads(client).length).toBe(2));
    expect(nftReads(client)[1]!.get("minEstateSize")).toBeNull();
  });

  it("draws the price histogram and range slider from /v1/prices, in MANA", async () => {
    const client = renderLands("web3", "/shop/lands", {
      "GET /shop-api/v1/nfts": nfts([]),
      "GET /shop-api/v1/prices": { body: { data: { [(2500n * MANA).toString()]: 12, [(40_000n * MANA).toString()]: 1 } } },
    });
    expect(await screen.findByRole("img", { name: "Price distribution" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Min price" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Max price" })).toBeInTheDocument();
    const prices = client.requests.map((r) => new URL(r.url)).find((u) => u.pathname === "/shop-api/v1/prices")!;
    expect(prices.searchParams.get("category")).toBe("land");
    noCredits();
  });

  it("loads the next page as the grid scrolls instead of paging, with skeletons below", async () => {
    let observed: ((entries: { isIntersecting: boolean }[]) => void) | null = null;
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        constructor(cb: (entries: { isIntersecting: boolean }[]) => void) {
          observed = cb;
        }
        observe() {}
        disconnect() {}
        unobserve() {}
      },
    );
    const page = (n: number) => Array.from({ length: 24 }, (_, i) => parcelResult({ x: n * 100 + i, y: 0 }));
    const client = renderLands("web3", "/shop/lands", {
      "GET /shop-api/v1/nfts": (_req, url) => ({ body: { data: page(url.searchParams.get("skip") === "0" ? 0 : 1), total: 60 } }),
    });
    await waitFor(() => expect(document.querySelectorAll(".shop-lb-card").length).toBe(24));
    expect(screen.queryByRole("navigation", { name: "Pages" })).toBeNull();
    expect(document.querySelectorAll(".shop-lb-skeleton").length).toBe(4);
    await waitFor(() => expect(observed).toBeTypeOf("function"));
    act(() => observed!([{ isIntersecting: true }]));
    await waitFor(() => expect(document.querySelectorAll(".shop-lb-card").length).toBe(48));
    expect(nftReads(client).at(-1)!.get("skip")).toBe("24");
    vi.unstubAllGlobals();
  });

  it("says the index is catching up instead of showing an empty market", async () => {
    renderLands("web3", "/shop/lands?isMap=false", { "GET /shop-api/v1/nfts": nfts([], 503) });
    expect(await screen.findByText(/nft-indexer is catching up.*not an empty market/)).toBeInTheDocument();
    expect(screen.queryByText("No LAND matches these filters")).toBeNull();
  });

  it("shows a load failure with a retry", async () => {
    renderLands("web3", "/shop/lands?isMap=false", { "GET /shop-api/v1/nfts": { status: 500, body: { error: "boom" } } });
    expect(await screen.findByText("The LAND listings could not be loaded.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });

  it("draws the map from the atlas tiles, and an unmirrored grid is an outage with a way to the list", async () => {
    renderLands("web3", "/shop/lands?isMap=true", {});
    expect(await screen.findByRole("region", { name: /LAND map: drag/ })).toBeInTheDocument();
    expect(document.querySelectorAll(".shop-landmap__sale").length).toBe(1);
    fireEvent.click(screen.getByRole("checkbox", { name: "Available to rent" }));
    await waitFor(() => expect(document.querySelectorAll(".shop-landmap__sale").length).toBe(2));
    fireEvent.click(screen.getByRole("checkbox", { name: "Available to buy" }));
    await waitFor(() => expect(document.querySelectorAll(".shop-landmap__sale").length).toBe(1));
  });

  it("reports a map that is not ready", async () => {
    renderLands("web3", "/shop/lands?isMap=true", {}, async () => {
      throw new MapNotReadyError();
    });
    expect(await screen.findByText("The LAND map is catching up and cannot be shown yet.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show the list instead" }));
    await screen.findByRole("region", { name: "LAND results" });
  });

  it("tells a session without a web3 wallet what LAND needs, without mentioning credits", async () => {
    renderLands("web2", "/shop/lands?isMap=false", { "GET /shop-api/v1/nfts": nfts([]) });
    expect(await screen.findByRole("note")).toHaveTextContent("LAND, estates and rentals are sold in MANA. Buying or renting LAND needs a web3 wallet holding MANA on Ethereum, and some ETH there to pay the network fee.");
    noCredits();
  });

  it("shows no wallet notice to a web3 session", async () => {
    renderLands("web3", "/shop/lands?isMap=false", { "GET /shop-api/v1/nfts": nfts([]) });
    await screen.findByText("No LAND matches these filters");
    expect(screen.queryByRole("note")).toBeNull();
  });
});

describe("/shop/token/:contract/:tokenId/manage", () => {
  const route = "/shop/token/:contractAddress/:tokenId/manage";
  const renderManage = (session: SessionKind, result: ReturnType<typeof parcelResult> | null, extra: Record<string, Answer> = {}, wallet?: ReturnType<typeof fakeWallet>, account = OWNER) => {
    const client = stubClient({ "GET /shop-api/v1/nfts": nfts(result ? [result] : []), ...extra });
    const contract = result?.nft.contractAddress ?? NET.land;
    const token = result?.nft.tokenId ?? "1";
    render(
      <ShopHarness session={session} account={account} provider={wallet ?? null} path={`/shop/token/${contract}/${token}/manage`} route={route}>
        <ManageAsset client={client} loadTiles={tiles} portal={false} timing={timing} />
      </ShopHarness>,
    );
    return client;
  };

  it("shows the owner's sell and rent boxes, the Builder boundary and the highlights", async () => {
    const result = parcelResult({ order: saleOrder((4000n * MANA).toString()) });
    renderManage("web3", result, {}, fakeWallet({ account: OWNER, chainId: 1 }));
    expect(await screen.findByRole("heading", { name: "Selling" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Edit listing" })).toHaveAttribute("href", `/shop/token/${NET.land}/${result.nft.tokenId}/sell`);
    expect(screen.getByRole("link", { name: "Open in Builder" })).toHaveAttribute("href", "/create/land");
    expect(screen.getByRole("link", { name: "Transfer" })).toHaveAttribute("href", `/shop/token/${NET.land}/${result.nft.tokenId}/transfer`);
    expect(screen.getByRole("button", { name: "List for rent" })).toBeEnabled();
    const highlights = await screen.findByRole("region", { name: "Highlights" });
    expect(within(highlights).getByText("Plaza")).toBeInTheDocument();
    noCredits();
  });

  it("refuses a visitor who does not own the LAND", async () => {
    renderManage("web3", parcelResult(), {}, fakeWallet({ account: TENANT, chainId: 1 }), TENANT);
    expect(await screen.findByText(/You are not the owner of Parcel 12,-4/)).toBeInTheDocument();
  });

  it("asks a signed-out visitor to sign in", async () => {
    const onSignIn = vi.fn();
    const client = stubClient({ "GET /shop-api/v1/nfts": nfts([parcelResult()]) });
    render(
      <ShopHarness session="signedOut" path={`/shop/token/${NET.land}/1/manage`} route={route} host={{ onSignIn }}>
        <ManageAsset client={client} loadTiles={tiles} />
      </ShopHarness>,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Connect" }));
    expect(onSignIn).toHaveBeenCalled();
  });

  it("explains the index outage", async () => {
    const client = stubClient({ "GET /shop-api/v1/nfts": nfts([], 503) });
    render(
      <ShopHarness session="web3" account={OWNER} path={`/shop/token/${NET.land}/1/manage`} route={route}>
        <ManageAsset client={client} loadTiles={tiles} />
      </ShopHarness>,
    );
    expect(await screen.findByText("This asset cannot be shown right now: nft-indexer is catching up")).toBeInTheDocument();
  });

  it("tells a web2 owner that LAND transactions need a web3 wallet and disables them", async () => {
    renderManage("web2", parcelResult());
    expect(await screen.findByRole("note")).toHaveTextContent("need a web3 wallet");
    expect(screen.getByRole("button", { name: "List for rent" })).toBeDisabled();
  });

  it("lists the LAND for rent: approval check, form, confirmation, signed post", async () => {
    const wallet = fakeWallet({
      account: OWNER,
      chainId: 1,
      reads: [
        { abi: erc721Abi, name: "isApprovedForAll", answer: () => true },
        { abi: rentalsAbi, name: "getContractIndex", answer: () => 0n },
        { abi: rentalsAbi, name: "getSignerIndex", answer: () => 0n },
        { abi: rentalsAbi, name: "getAssetIndex", answer: () => 0n },
      ],
    });
    const client = renderManage("web3", parcelResult(), { "POST /shop-api/v1/rentals-listings": { body: { ok: true, data: rentalListing() } } }, wallet);
    fireEvent.click(await enabledButton("List for rent"));
    const dialog = await screen.findByRole("dialog", { name: "List for Rent" });
    await within(dialog).findByLabelText(/Price per day/);
    fireEvent.change(within(dialog).getByLabelText(/Price per day/), { target: { value: "25" } });
    fireEvent.click(within(dialog).getByRole("checkbox", { name: "30 days" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "List for rent" }));
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));
    expect(await screen.findByText("Parcel 12,-4 is listed for rent.")).toBeInTheDocument();
    const post = client.requests.find((r) => r.method === "POST")!;
    expect(JSON.parse(post.body!).periods).toEqual([{ minDays: 30, maxDays: 30, pricePerDay: (25n * MANA).toString() }]);
    expect(wallet.signed).toHaveLength(1);
  });

  it("asks for the Rentals operator approval before the listing form", async () => {
    const wallet = fakeWallet({ account: OWNER, chainId: 1, reads: [{ abi: erc721Abi, name: "isApprovedForAll", answer: () => false }], writes: [erc721Abi] });
    renderManage("web3", parcelResult(), {}, wallet);
    fireEvent.click(await enabledButton("List for rent"));
    fireEvent.click(await screen.findByRole("button", { name: "Proceed" }));
    await waitFor(() => expect(wallet.sent[0]).toMatchObject({ fn: "setApprovalForAll", to: NET.land, args: [NET.rentals, true] }));
  });

  it("claims a LAND whose listing ended while the Rentals contract holds it", async () => {
    const rental = rentalListing({ status: "cancelled" });
    const result = parcelResult({ owner: NET.rentals, rental });
    const wallet = fakeWallet({ account: OWNER, chainId: 1, writes: [rentalsAbi] });
    renderManage("web3", result, { "PATCH /shop-api/v1/rentals-listings/rental-1": { body: { ok: true, data: { ...rental, status: "claimed" } } } }, wallet);
    expect(await screen.findByText(/still unclaimed/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Transfer" })).toBeDisabled();
    fireEvent.click(await enabledButton("Claim Parcel"));
    const dialog = await screen.findByRole("dialog", { name: "Claim Parcel" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Claim Parcel" }));
    expect(await screen.findByText("Parcel 12,-4 is back in your wallet.")).toBeInTheDocument();
    expect(wallet.sent[0]).toMatchObject({ fn: "claim" });
  });

  it("sends a NAME to the Creator Hub for its World", async () => {
    const client = stubClient({ "GET /shop-api/v1/nfts": nfts([nameResult()]) });
    render(
      <ShopHarness session="web3" account={OWNER} path={`/shop/token/${NET.nameRegistrar}/1/manage`} route={route}>
        <ManageAsset client={client} />
      </ShopHarness>,
    );
    expect(await screen.findByRole("link", { name: "Manage your NAMEs" })).toHaveAttribute("href", "/create/names");
  });
});

describe("/shop/token/:contract/:tokenId/rent", () => {
  const route = "/shop/token/:contractAddress/:tokenId/rent";
  const renting = () => parcelResult({ rental: rentalListing() });
  const renderRent = (session: SessionKind, result: ReturnType<typeof parcelResult>, opts: { wallet?: ReturnType<typeof fakeWallet>; extra?: Record<string, Answer>; account?: string; host?: Record<string, unknown> } = {}) => {
    const client = stubClient({ "GET /shop-api/v1/nfts": nfts([result]), ...opts.extra });
    const reader = opts.wallet ? readerOf(opts.wallet) : undefined;
    render(
      <ShopHarness session={session} account={opts.account ?? TENANT} provider={opts.wallet ?? null} path={`/shop/token/${result.nft.contractAddress}/${result.nft.tokenId}/rent`} route={route} host={opts.host}>
        <RentPage client={client} loadTiles={tiles} reader={reader} timing={timing} />
      </ShopHarness>,
    );
    return client;
  };

  it("rents for a chosen period in MANA: exact total, price re-entry, allowance, acceptListing, confirmation", async () => {
    const wallet = fakeWallet({
      account: TENANT,
      chainId: 1,
      reads: [
        { abi: erc20Abi, name: "balanceOf", answer: () => 100n * MANA },
        { abi: erc20Abi, name: "allowance", answer: () => 0n },
      ],
      writes: [rentalsAbi],
    });
    const result = renting();
    renderRent("web3", result, { wallet, extra: { "PATCH /shop-api/v1/rentals-listings/rental-1": { body: { ok: true, data: rentalListing({ status: "executed", startedAt: Date.now(), rentedDays: 30, tenant: TENANT }) } } } });
    fireEvent.click(await screen.findByRole("radio", { name: "30 days" }));
    expect(screen.getByRole("region", { name: "Pay with MANA" })).toHaveTextContent("Total30");
    fireEvent.click(await enabledButton("Rent"));
    fireEvent.change(screen.getByLabelText("Price (MANA)"), { target: { value: "30" } });
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(await screen.findByRole("heading", { name: "LAND rented" })).toBeInTheDocument();
    expect(wallet.sent.map((t) => t.fn)).toEqual(["approve", "acceptListing"]);
    expect(screen.getByRole("link", { name: /Manage LAND/ })).toHaveAttribute("href", "/create/land");
    noCredits();
  });

  it("keeps Confirm disabled until the re-entered price matches", async () => {
    const wallet = fakeWallet({ account: TENANT, chainId: 1, reads: [{ abi: erc20Abi, name: "balanceOf", answer: () => 100n * MANA }] });
    renderRent("web3", renting(), { wallet });
    fireEvent.click(await screen.findByRole("radio", { name: "7 days" }));
    fireEvent.click(await enabledButton("Rent"));
    fireEvent.change(screen.getByLabelText("Price (MANA)"), { target: { value: "13" } });
    expect(screen.getByRole("button", { name: "Confirm" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Price (MANA)"), { target: { value: "14" } });
    expect(screen.getByRole("button", { name: "Confirm" })).toBeEnabled();
  });

  it("says a web3 wallet does not hold enough MANA", async () => {
    const wallet = fakeWallet({ account: TENANT, chainId: 1, reads: [{ abi: erc20Abi, name: "balanceOf", answer: () => 5n * MANA }] });
    renderRent("web3", renting(), { wallet });
    fireEvent.click(await screen.findByRole("radio", { name: "30 days" }));
    expect(await screen.findByText("You don't have enough MANA to rent this LAND for that period.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rent" })).toBeDisabled();
    noCredits();
  });

  it("asks a wallet on the wrong network to switch, from a click", async () => {
    const wallet = fakeWallet({ account: TENANT, chainId: 137, reads: [{ abi: erc20Abi, name: "balanceOf", answer: () => 100n * MANA }] });
    renderRent("web3", renting(), { wallet });
    fireEvent.click(await screen.findByRole("button", { name: "Switch to Ethereum" }));
    await waitFor(() => expect(wallet.chainId).toBe(1));
  });

  it("gives a web2 buyer the MANA guidance: network, exact balance, gas, the account's wallet, the item kept, no credits", async () => {
    const onConnectWallet = vi.fn();
    renderRent("web2", renting(), { host: { onConnectWallet } });
    fireEvent.click(await screen.findByRole("radio", { name: "30 days" }));
    const guidance = await screen.findByRole("region", { name: "This purchase is paid in MANA" });
    expect(guidance).toHaveTextContent("LAND, estates and rentals are sold in MANA.");
    expect(guidance).toHaveTextContent("30 MANA on Ethereum");
    expect(guidance).toHaveTextContent("Some ETH on Ethereum to pay the network fee (gas).");
    fireEvent.click(within(guidance).getByRole("button", { name: /Connect the wallet for/ }));
    expect(onConnectWallet).toHaveBeenCalled();
    expect(JSON.parse(sessionStorage.getItem("dcl-shop-wallet-return:v1")!)).toMatchObject({ itemHref: `/shop/token/${NET.land}/${renting().nft.tokenId}` });
    expect(screen.queryByRole("button", { name: "Rent" })).toBeNull();
    noCredits();
  });

  it("asks a signed-out visitor to sign in with a wallet", async () => {
    renderRent("signedOut", renting());
    fireEvent.click(await screen.findByRole("radio", { name: "7 days" }));
    expect(await screen.findByRole("button", { name: "Sign in with a wallet" })).toBeInTheDocument();
  });

  it("does not offer a parcel of an estate, an unlisted LAND or the owner's own", async () => {
    renderRent("web3", parcelResult({ rental: rentalListing(), estate: { name: "Big", tokenId: "9" } }), { wallet: fakeWallet({ account: TENANT, chainId: 1 }) });
    expect(await screen.findByText(/not possible to rent a parcel that belongs to an Estate/)).toBeInTheDocument();
  });

  it("says when the LAND is not listed for rent", async () => {
    renderRent("web3", parcelResult(), { wallet: fakeWallet({ account: TENANT, chainId: 1 }) });
    expect(await screen.findByText(/not listed for rent right now/)).toBeInTheDocument();
  });

  it("recognizes the owner", async () => {
    renderRent("web3", renting(), { wallet: fakeWallet({ account: OWNER, chainId: 1 }), account: OWNER });
    expect(await screen.findByRole("link", { name: "Manage its rental listing" })).toBeInTheDocument();
  });

  it("refuses to sign when the wallet moved to another account, and says whose LAND it would be", async () => {
    const wallet = fakeWallet({ account: TENANT, chainId: 1, reads: [{ abi: erc20Abi, name: "balanceOf", answer: () => 100n * MANA }] });
    renderRent("web3", renting(), { wallet });
    fireEvent.click(await screen.findByRole("radio", { name: "30 days" }));
    await act(async () => wallet.emit("accountsChanged", ["0x00000000000000000000000000000000000000cc"]));
    expect(await screen.findByText(/Your wallet switched to 0x00000000000000000000000000000000000000cc/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rent" })).toBeDisabled();
  });

  it("binds an estate rental to the registry fingerprint", async () => {
    const fp = `0x${"cd".repeat(32)}`;
    const wallet = fakeWallet({
      account: TENANT,
      chainId: 1,
      reads: [
        { abi: erc20Abi, name: "balanceOf", answer: () => 100n * MANA },
        { abi: erc20Abi, name: "allowance", answer: () => 100n * MANA },
        { abi: estateRegistryAbi, name: "getFingerprintV2", answer: () => fp },
      ],
      writes: [rentalsAbi],
    });
    const result = estateResult({ rental: rentalListing({ contractAddress: NET.estate, tokenId: "4213" }) });
    renderRent("web3", result, { wallet, extra: { "PATCH /shop-api/v1/rentals-listings/rental-1": { body: { ok: true, data: rentalListing({ status: "executed" }) } } } });
    fireEvent.click(await screen.findByRole("radio", { name: "7 days" }));
    fireEvent.click(await enabledButton("Rent"));
    fireEvent.change(screen.getByLabelText("Price (MANA)"), { target: { value: "14" } });
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    await screen.findByRole("heading", { name: "LAND rented" });
    expect((wallet.sent[0]!.args as unknown[])[4]).toBe(fp);
  });

  it("shows a declined wallet request as an answer, with a way back", async () => {
    const wallet = fakeWallet({ account: TENANT, chainId: 1, reads: [{ abi: erc20Abi, name: "balanceOf", answer: () => 100n * MANA }, { abi: erc20Abi, name: "allowance", answer: () => 100n * MANA }], writes: [rentalsAbi] });
    wallet.reject.add("eth_sendTransaction");
    renderRent("web3", renting(), { wallet });
    fireEvent.click(await screen.findByRole("radio", { name: "7 days" }));
    fireEvent.click(await enabledButton("Rent"));
    fireEvent.change(screen.getByLabelText("Price (MANA)"), { target: { value: "14" } });
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(await screen.findByText("You declined the request in your wallet. Nothing was sent.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });
});

import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BOOK } from "../payment/fixtures";
import { createDemo, DemoShell, type DemoScenario } from "./demo";
import SettingsPage from "./SettingsPage";

function renderSettings(s: Partial<DemoScenario> = {}) {
  const demo = createDemo({
    session: "web3",
    handlers: { nfts: () => ({ data: [{ nft: { contractAddress: BOOK.collection, chainId: 137 } }] }) },
    ...s,
  });
  render(
    <DemoShell demo={demo} path="/shop/settings">
      <SettingsPage />
    </DemoShell>,
  );
  return demo;
}

const rowOf = (name: RegExp, chain: string) =>
  screen.getAllByTestId("authorization").find((r) => name.test(r.textContent ?? "") && (r.textContent ?? "").includes(`on ${chain}`))!;

describe("the settings page (authorizations)", () => {
  it("signed out: draws the Marketplace's Get Started sign-in page", () => {
    renderSettings({ session: "signedOut" });
    expect(screen.getByTestId("settings-signed-out")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Get Started" })).toBeTruthy();
  });

  it("shows the address row and upstream's authorization groups", async () => {
    renderSettings();
    expect(screen.getByText("Copy address")).toBeTruthy();
    for (const g of ["For buying", "For bidding", "For renting"]) expect(screen.getByRole("group", { name: g })).toBeTruthy();
    expect(within(screen.getByRole("group", { name: "For bidding" })).getAllByTestId("authorization")).toHaveLength(2);
    expect(within(screen.getByRole("group", { name: "For renting" })).getAllByTestId("authorization")[0]!.textContent).toContain("Rentals");
  });

  it("lists MANA allowances per chain and the granted selling approvals of held collections, read from the chain", async () => {
    const demo = createDemo({ session: "web3", handlers: { nfts: () => ({ data: [{ nft: { contractAddress: BOOK.collection, chainId: 137 } }] }) } });
    demo.chain.allowances.set(`137:${BOOK.buyer}:0xe38ef22abe871513555cba89adfe45ab4f548ada`, 10n ** 30n);
    demo.chain.approvals.set(`137:${BOOK.collection.toLowerCase()}:${BOOK.buyer}:0xe38ef22abe871513555cba89adfe45ab4f548ada`, true);
    render(
      <DemoShell demo={demo} path="/shop/settings">
        <SettingsPage />
      </DemoShell>,
    );
    await waitFor(() => expect(rowOf(/OffChainMarketplaceV3/, "Polygon").textContent).toContain("Authorized"));
    expect(rowOf(/CollectionStore/, "Polygon").textContent).toContain("Not authorized");
    expect(screen.getAllByTestId("authorization").some((r) => (r.textContent ?? "").includes("Ethereum"))).toBe(true);
    await waitFor(() => expect(screen.getByRole("group", { name: "For selling" })).toBeTruthy());
    expect(within(screen.getByRole("group", { name: "For selling" })).getAllByTestId("authorization")).toHaveLength(1);
  });

  it("a Polygon allowance is granted relayed, from the buyer's click", async () => {
    const demo = renderSettings();
    const row = await waitFor(() => {
      const r = rowOf(/CollectionStore/, "Polygon");
      expect(r.textContent).toContain("Not authorized");
      return r;
    });
    await act(async () => fireEvent.click(within(row).getByRole("switch")));
    await waitFor(() => expect(rowOf(/CollectionStore/, "Polygon").textContent).toContain("Authorized"));
    expect(demo.relay.posts).toHaveLength(1);
  });

  it("an Ethereum change needs the wallet on Ethereum: it asks to switch, never switches by itself", async () => {
    const demo = renderSettings();
    const row = await waitFor(() => {
      const r = rowOf(/OffChainMarketplaceV3/, "Ethereum");
      expect(r.textContent).toContain("Not authorized");
      return r;
    });
    await act(async () => fireEvent.click(within(row).getByRole("switch")));
    const alert = await within(rowOf(/OffChainMarketplaceV3/, "Ethereum")).findByRole("alert");
    expect(alert.textContent).toContain("Switch your wallet to Ethereum");
    expect(demo.wallet.chainId).toBe(137);
    await act(async () => fireEvent.click(within(alert).getByRole("button", { name: "Switch to Ethereum" })));
    expect(demo.wallet.chainId).toBe(1);
  });

  it("a managed wallet sees them read-only", async () => {
    renderSettings({ session: "web2" });
    expect(screen.getByText("Handled for you")).toBeTruthy();
    await waitFor(() => expect(screen.getAllByRole("switch").length).toBeGreaterThan(0));
    expect(screen.getAllByRole("switch").every((b) => (b as HTMLButtonElement).disabled)).toBe(true);
  });
});

// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const api = vi.hoisted(() => ({ fetchManaBalance: vi.fn(), fetchManaTransfers: vi.fn(), ownWalletOf: vi.fn() }));
vi.mock("@/data/lib/account/wallet", async (original) => ({
  ...(await original<typeof import("@/data/lib/account/wallet")>()),
  fetchManaBalance: api.fetchManaBalance,
  fetchManaTransfers: api.fetchManaTransfers,
  ownWalletOf: api.ownWalletOf,
}));

import { accountKeys } from "@/data/lib/account/queries";
import { CatalystError } from "@/data/lib/catalyst/client";
import type { AuthIdentity } from "@/data/lib/auth/types";
import type { ManaTransfer, WalletNetwork } from "@/data/lib/account/wallet";

import { WalletsSettings } from "./WalletsSettings";

const SIGNER = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const OTHER = "0x1111111111111111111111111111111111111111";
const identity: AuthIdentity = {
  signer: SIGNER,
  ephemeral: { address: "0xeph", privateKey: "0x01" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};

const transfer = (n: number, over: Partial<ManaTransfer> = {}): ManaTransfer => ({
  id: `100:${n}`,
  hash: `0x${n.toString(16).padStart(64, "0")}`,
  direction: "sent",
  counterparty: OTHER,
  amount: "2500000000000000000",
  at: Date.UTC(2026, 10, 2, 12, 0, 0),
  ...over,
});

let client: QueryClient;

function show() {
  return render(
    <QueryClientProvider client={client}>
      <WalletsSettings identity={identity} />
    </QueryClientProvider>,
  );
}

const card = (name: string) => screen.getByRole("region", { name });
const balances = (amounts: Partial<Record<WalletNetwork, string | Error>>) =>
  api.fetchManaBalance.mockImplementation((_account: string, network: WalletNetwork) => {
    const amount = amounts[network] ?? "0";
    return amount instanceof Error ? Promise.reject(amount) : Promise.resolve(amount);
  });

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
});
afterEach(() => {
  cleanup();
  client.clear();
  vi.resetAllMocks();
});

describe("WalletsSettings", () => {
  it("groups both network cards in the Wallets panel", async () => {
    balances({});
    show();

    const panel = card("Wallets");
    expect(within(panel).getByRole("region", { name: "Ethereum MANA" })).toBeTruthy();
    expect(within(panel).getByRole("region", { name: "Polygon MANA" })).toBeTruthy();
    expect(within(panel).queryByRole("button", { name: "Copy address" })).toBeNull();
    await within(card("Ethereum MANA")).findByText("0");
  });

  it("shows each network's MANA balance, read for the signed-in account", async () => {
    balances({ ethereum: "100595000000000000000000", polygon: "1239900000000000000" });
    show();

    expect(await within(card("Ethereum MANA")).findByText("100,595")).toBeTruthy();
    expect(await within(card("Polygon MANA")).findByText("1.23")).toBeTruthy();
    expect(api.fetchManaBalance.mock.calls.map(([account, network]) => [account, network])).toEqual([
      [SIGNER, "ethereum"],
      [SIGNER, "polygon"],
    ]);
  });

  it("says a balance is not available, never zero, when it cannot be read, and reads it again on Try again", async () => {
    const user = userEvent.setup();
    balances({ ethereum: new Error("unsupported network"), polygon: "5000000000000000000" });
    show();

    const ethereum = card("Ethereum MANA");
    expect(await within(ethereum).findByText(/^Balance not available/)).toBeTruthy();
    expect(within(ethereum).queryByText("0")).toBeNull();
    expect(await within(card("Polygon MANA")).findByText("5")).toBeTruthy();

    balances({ ethereum: "7000000000000000000", polygon: "5000000000000000000" });
    await user.click(within(ethereum).getByRole("button", { name: "Try again, Ethereum MANA balance" }));

    expect(await within(ethereum).findByText("7")).toBeTruthy();
    expect(within(ethereum).getByRole("status").textContent).toBe("");
  });

  it("keeps an amount already shown and marks it stale after a failed refresh", async () => {
    balances({ ethereum: "7000000000000000000", polygon: "5000000000000000000" });
    show();

    const ethereum = card("Ethereum MANA");
    expect(await within(ethereum).findByText("7")).toBeTruthy();

    balances({ ethereum: new Error("rpc down"), polygon: "5000000000000000000" });
    await act(() => client.refetchQueries({ queryKey: accountKeys.manaBalance(SIGNER, "ethereum") }));

    expect(await within(ethereum).findByText(/balance shown may be out of date/)).toBeTruthy();
    expect(within(ethereum).getByText("7")).toBeTruthy();
    expect(within(ethereum).getByRole("button", { name: "Try again, Ethereum MANA balance" })).toBeTruthy();
  });

  it("offers no control that moves MANA or asks for a signature", async () => {
    balances({ ethereum: "1000000000000000000", polygon: "1000000000000000000" });
    show();
    await within(card("Polygon MANA")).findByText("1");

    expect(screen.getAllByRole("button").map((button) => button.getAttribute("aria-label") ?? button.textContent)).toEqual([
      "Transactions, Ethereum MANA",
      "Transactions, Polygon MANA",
    ]);
    expect(screen.queryAllByRole("link")).toEqual([]);
    expect(api.ownWalletOf).not.toHaveBeenCalled();
  });

  it("reads no history until the member opens a network's transactions", async () => {
    const user = userEvent.setup();
    balances({});
    api.fetchManaTransfers.mockResolvedValue({ transfers: [transfer(1)], next: null });
    show();
    await within(card("Polygon MANA")).findByText("0");

    const toggle = screen.getByRole("button", { name: "Transactions, Polygon MANA" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(api.fetchManaTransfers).not.toHaveBeenCalled();

    await user.click(toggle);

    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(await screen.findByRole("list", { name: "Transactions, Polygon MANA" })).toBeTruthy();
    expect(api.fetchManaTransfers).toHaveBeenCalledTimes(1);
    expect(api.fetchManaTransfers.mock.calls[0]!.slice(0, 2)).toEqual([SIGNER, "polygon"]);
  });

  it("lists confirmed transfers with their direction, amount, time and Ethereum explorer links", async () => {
    const user = userEvent.setup();
    balances({});
    api.fetchManaTransfers.mockResolvedValue({
      transfers: [transfer(1), transfer(2, { direction: "received", amount: "1" })],
      next: null,
    });
    show();

    await user.click(screen.getByRole("button", { name: "Transactions, Ethereum MANA" }));

    const rows = within(await screen.findByRole("list", { name: "Transactions, Ethereum MANA" })).getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain("Sent");
    expect(rows[0]!.textContent).toContain("2.5 MANA");
    expect(within(rows[0]!).getByText("Confirmed")).toBeTruthy();
    expect(within(rows[0]!).getByRole("link", { name: `To ${OTHER}` }).getAttribute("href")).toBe(
      `https://etherscan.io/address/${OTHER}`,
    );
    expect(within(rows[0]!).getByRole("link", { name: transfer(1).hash }).getAttribute("href")).toBe(
      `https://etherscan.io/tx/${transfer(1).hash}`,
    );
    expect(rows[0]!.querySelector("time")?.getAttribute("datetime")).toBe("2026-11-02T12:00:00.000Z");
    expect(rows[1]!.textContent).toContain("Received");
    expect(rows[1]!.textContent).toContain("< 0.01 MANA");
    expect(within(rows[1]!).getByText("Confirmed")).toBeTruthy();
    expect(within(rows[1]!).getByRole("link", { name: `From ${OTHER}` })).toBeTruthy();
    expect(within(card("Ethereum MANA")).getByText("Latest transactions")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
  });

  describe("when Polygon history is populated", () => {
    beforeEach(() => {
      balances({});
      api.fetchManaTransfers.mockResolvedValue({ transfers: [transfer(1)], next: null });
    });

    it("should link the transaction hash to PolygonScan", async () => {
      show();
      await userEvent.click(screen.getByRole("button", { name: "Transactions, Polygon MANA" }));

      const link = await within(card("Polygon MANA")).findByRole("link", { name: transfer(1).hash });
      expect(link.getAttribute("href")).toBe(`https://polygonscan.com/tx/${transfer(1).hash}`);
    });
  });

  it("says there are no transactions only when the node read an empty history", async () => {
    const user = userEvent.setup();
    balances({});
    api.fetchManaTransfers.mockResolvedValue({ transfers: [], next: null });
    show();

    await user.click(screen.getByRole("button", { name: "Transactions, Ethereum MANA" }));

    expect(await within(card("Ethereum MANA")).findByText("No transactions yet.")).toBeTruthy();
  });

  it.each([
    [new CatalystError("MANA transfer history is synchronizing", "u", 503, true), "Transaction history is not available on this node."],
    [new CatalystError("HTTP 503", "u", 503), "We couldn't load your transactions. Please try again later."],
    [new Error("offline"), "We couldn't load your transactions. Please try again later."],
  ])("says the history could not be read (%s), not that it is empty, and reads it again on Try again", async (failure, message) => {
    const user = userEvent.setup();
    balances({});
    api.fetchManaTransfers.mockRejectedValueOnce(failure).mockResolvedValue({ transfers: [transfer(1)], next: null });
    show();
    const ethereum = card("Ethereum MANA");

    await user.click(screen.getByRole("button", { name: "Transactions, Ethereum MANA" }));

    expect(await within(ethereum).findByText(message)).toBeTruthy();
    expect(within(ethereum).queryByText("No transactions yet.")).toBeNull();
    expect(ethereum.textContent).not.toContain("synchronizing");

    await user.click(within(ethereum).getByRole("button", { name: "Try again, Ethereum MANA transactions" }));

    expect(await screen.findByRole("list", { name: "Transactions, Ethereum MANA" })).toBeTruthy();
  });

  describe("when retrying history after a temporary failure", () => {
    let finishHistory: (page: { transfers: ManaTransfer[]; next: null }) => void;

    beforeEach(() => {
      balances({});
      const response = new Promise<{ transfers: ManaTransfer[]; next: null }>((resolve) => {
        finishHistory = resolve;
      });
      api.fetchManaTransfers.mockRejectedValueOnce(new Error("offline")).mockReturnValueOnce(response);
    });

    it("should announce progress and then render the recovered history", async () => {
      show();
      await userEvent.click(screen.getByRole("button", { name: "Transactions, Ethereum MANA" }));
      await userEvent.click(await within(card("Ethereum MANA")).findByRole("button", { name: "Try again, Ethereum MANA transactions" }));

      expect(await screen.findByText("Loading transactions…")).toBeTruthy();
      finishHistory({ transfers: [transfer(1)], next: null });
      expect(await screen.findByRole("list", { name: "Transactions, Ethereum MANA" })).toBeTruthy();
    });
  });

  describe("when balance and history refreshes both fail", () => {
    beforeEach(() => {
      balances({});
      api.fetchManaTransfers.mockResolvedValueOnce({ transfers: [transfer(1)], next: null });
    });

    it("should keep both values visible and name each retry", async () => {
      show();
      const ethereum = card("Ethereum MANA");
      await within(ethereum).findByText("0");
      await userEvent.click(within(ethereum).getByRole("button", { name: "Transactions, Ethereum MANA" }));
      const list = await within(ethereum).findByRole("list");

      balances({ ethereum: new Error("offline") });
      api.fetchManaTransfers.mockRejectedValueOnce(new Error("offline"));
      await act(() => client.refetchQueries({ queryKey: accountKeys.all(SIGNER) }));

      expect(within(ethereum).getByText("0")).toBeTruthy();
      expect(within(list).getAllByRole("listitem")).toHaveLength(1);
      expect(await within(ethereum).findByText(/history shown may be out of date/)).toBeTruthy();
      expect(within(ethereum).getByRole("button", { name: "Try again, Ethereum MANA balance" })).toBeTruthy();
      expect(within(ethereum).getByRole("button", { name: "Try again, Ethereum MANA transactions" })).toBeTruthy();
    });
  });

  it("adds the next page from Load more, and keeps the rows shown when that read fails", async () => {
    const user = userEvent.setup();
    balances({});
    const cursor = { beforeBlock: 100, beforeLog: 1 };
    api.fetchManaTransfers
      .mockResolvedValueOnce({ transfers: [transfer(1)], next: cursor })
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce({ transfers: [transfer(2)], next: null });
    show();

    await user.click(screen.getByRole("button", { name: "Transactions, Ethereum MANA" }));
    const list = await screen.findByRole("list", { name: "Transactions, Ethereum MANA" });
    await user.click(screen.getByRole("button", { name: "Load more" }));

    expect((await screen.findByRole("alert")).textContent).toContain("We couldn't load more transactions");
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);

    await user.click(screen.getByRole("button", { name: "Load more" }));

    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(2));
    expect(api.fetchManaTransfers.mock.calls.at(-1)![2]).toMatchObject({ cursor });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
  });

  it("closes the list again from the same control", async () => {
    const user = userEvent.setup();
    balances({});
    api.fetchManaTransfers.mockResolvedValue({ transfers: [transfer(1)], next: null });
    show();
    const toggle = screen.getByRole("button", { name: "Transactions, Ethereum MANA" });

    await user.click(toggle);
    await screen.findByRole("list", { name: "Transactions, Ethereum MANA" });
    await user.click(toggle);

    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("list", { name: "Transactions, Ethereum MANA" })).toBeNull();
  });
});

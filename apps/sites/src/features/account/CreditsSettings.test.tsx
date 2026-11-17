// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const api = vi.hoisted(() => ({ fetchCreditsStatus: vi.fn(), optOutOfCredits: vi.fn() }));
vi.mock("@/data/lib/account/credits", async (original) => ({
  ...(await original<typeof import("@/data/lib/account/credits")>()),
  fetchCreditsStatus: api.fetchCreditsStatus,
  optOutOfCredits: api.optOutOfCredits,
}));

import { CatalystError } from "@/data/lib/catalyst/client";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { CreditsSettings } from "./CreditsSettings";

const SIGNER = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const identity: AuthIdentity = {
  signer: SIGNER,
  ephemeral: { address: "0xeph", privateKey: "0x01" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};
const enrolled = { status: "enrolled", optedOutAt: null } as const;
const optedOut = { status: "opted_out", optedOutAt: "2026-11-16T00:00:00.000Z" } as const;

let client: QueryClient;

function show() {
  return render(
    <QueryClientProvider client={client}>
      <CreditsSettings identity={identity} />
    </QueryClientProvider>,
  );
}

async function askToLeave(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: "Leave Program" }));
  return screen.getByRole("group", { name: "Leave Marketplace Credits?" });
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
});
afterEach(() => {
  cleanup();
  client.clear();
  vi.resetAllMocks();
});

describe("CreditsSettings", () => {
  it("shows an enrolled member's status and a way to leave", async () => {
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    show();

    expect(await screen.findByText("Enrolled")).toBeTruthy();
    expect((screen.getByRole("link", { name: "Learn more" })).getAttribute("href")).toContain("/docs/");
    expect((screen.getByRole("button", { name: "Leave Program" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it.each([
    [optedOut, "Opted Out"],
    [{ status: "not_registered", optedOutAt: null }, "Not Registered"],
  ])("offers no way to leave when the member is not enrolled (%o)", async (status, label) => {
    api.fetchCreditsStatus.mockResolvedValue(status);
    show();

    expect(await screen.findByText(label)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Leave Program" })).toBeNull();
  });

  it("says so when the status cannot be read, offering nothing to leave", async () => {
    api.fetchCreditsStatus.mockRejectedValue(new Error("offline"));
    show();

    expect((await screen.findByRole("alert")).textContent).toContain("We couldn't load your credits status");
    expect(screen.queryByRole("button", { name: "Leave Program" })).toBeNull();
  });

  it("leaves nothing changed when the member cancels, and returns focus to Leave", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    show();

    const confirm = await askToLeave(user);
    expect(document.activeElement).toBe(confirm);
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(screen.queryByRole("group", { name: "Leave Marketplace Credits?" })).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Leave Program" }));
    expect(api.optOutOfCredits).not.toHaveBeenCalled();
    expect(screen.getByText("Enrolled")).toBeTruthy();
  });

  it("opts out on confirm and shows the status the server reads back", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValueOnce(enrolled).mockResolvedValue(optedOut);
    api.optOutOfCredits.mockResolvedValue(undefined);
    show();

    await askToLeave(user);
    await user.click(screen.getAllByRole("button", { name: "Leave Program" }).at(-1)!);

    await waitFor(() => expect(api.optOutOfCredits).toHaveBeenCalledWith(identity));
    expect(await screen.findByText("Opted Out")).toBeTruthy();
    expect(screen.queryByRole("group", { name: "Leave Marketplace Credits?" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Leave Program" })).toBeNull();
  });

  it("keeps the member enrolled and explains a refused opt-out without the server's text", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    api.optOutOfCredits.mockRejectedValue(new CatalystError("Cannot unregister with credits claimed this week", "u", 400, true));
    show();

    await askToLeave(user);
    await user.click(screen.getAllByRole("button", { name: "Leave Program" }).at(-1)!);

    expect((await screen.findByRole("alert")).textContent).toContain("You can't leave this week because you've already claimed credits");
    expect(screen.getByText("Enrolled")).toBeTruthy();
    expect(api.fetchCreditsStatus).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("group", { name: "Leave Marketplace Credits?" })).toBeTruthy();
  });

  it("shows a generic retry message for any other failure, cleared by cancelling", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    api.optOutOfCredits.mockRejectedValue(new CatalystError("database exploded", "u", 500, true));
    show();

    await askToLeave(user);
    await user.click(screen.getAllByRole("button", { name: "Leave Program" }).at(-1)!);

    const alert = await screen.findByRole("alert");
    expect((alert).textContent).toContain("Couldn't leave the program right now. Please try again.");
    expect(alert.textContent).not.toContain("database exploded");

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await askToLeave(user);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

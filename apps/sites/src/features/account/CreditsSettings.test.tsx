// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
  return screen.getByRole("dialog", { name: "Leave Marketplace Credits?" });
}

/** The confirmation's own Leave Program, not the card's one behind it. */
function confirmLeave(user: ReturnType<typeof userEvent.setup>, dialog: HTMLElement) {
  return user.click(within(dialog).getByRole("button", { name: "Leave Program" }));
}

// jsdom has no modal dialogs: opening and closing only toggle `open`, so the dialog's role shows.
const showModal = vi.fn(function (this: HTMLDialogElement) {
  this.open = true;
});
const closeDialog = vi.fn(function (this: HTMLDialogElement) {
  this.open = false;
});

const nativeShowModal = HTMLDialogElement.prototype.showModal;
const nativeClose = HTMLDialogElement.prototype.close;

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  HTMLDialogElement.prototype.showModal = showModal;
  HTMLDialogElement.prototype.close = closeDialog;
});
afterEach(() => {
  cleanup();
  HTMLDialogElement.prototype.showModal = nativeShowModal;
  HTMLDialogElement.prototype.close = nativeClose;
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

  it("reads the status again from Try again after a failed read", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockRejectedValueOnce(new Error("offline")).mockResolvedValue(enrolled);
    show();

    await user.click(await screen.findByRole("button", { name: "Try again" }));

    expect(await screen.findByText("Enrolled")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("button", { name: "Leave Program" })).toBeTruthy();
  });

  it("shows the failure, not a stale Enrolled with Leave, when the read after leaving fails", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValueOnce(enrolled).mockRejectedValue(new Error("offline"));
    api.optOutOfCredits.mockResolvedValue(undefined);
    show();

    await confirmLeave(user, await askToLeave(user));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("We couldn't load your credits status");
    await waitFor(() => expect(document.activeElement).toBe(alert));
    expect(screen.queryByText("Enrolled")).toBeNull();
    expect(screen.queryByRole("button", { name: "Leave Program" })).toBeNull();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("opens a modal confirmation that locks the page's scroll", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    show();

    await askToLeave(user);

    expect(showModal).toHaveBeenCalledTimes(1);
    expect(document.documentElement.style.overflow).toBe("hidden");
    expect(screen.getByRole("dialog").getAttribute("aria-describedby")).toBeTruthy();
  });

  it.each([
    ["Cancel", (dialog: HTMLElement, user: ReturnType<typeof userEvent.setup>) => user.click(within(dialog).getByRole("button", { name: "Cancel" }))],
    ["Close", (dialog: HTMLElement, user: ReturnType<typeof userEvent.setup>) => user.click(within(dialog).getByRole("button", { name: "Close" }))],
    ["Escape", async (dialog: HTMLElement) => void fireEvent.keyDown(dialog, { key: "Escape" })],
    ["the browser's cancel request", async (dialog: HTMLElement) => void fireEvent(dialog, new Event("cancel", { cancelable: true }))],
    ["a close the browser forces", async (dialog: HTMLElement) => {
      (dialog as HTMLDialogElement).open = false;
      fireEvent(dialog, new Event("close"));
    }],
    ["a backdrop click", async (dialog: HTMLElement) => {
      fireEvent.pointerDown(dialog, { clientX: -10, clientY: -10 });
      fireEvent.click(dialog, { clientX: -10, clientY: -10 });
    }],
  ])("dismisses with %s, changing nothing and returning focus to Leave", async (_how, dismiss) => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    show();

    const dialog = await askToLeave(user);
    await dismiss(dialog, user);

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Leave Program" }));
    expect(document.documentElement.style.overflow).toBe("");
    expect(api.optOutOfCredits).not.toHaveBeenCalled();
    expect(screen.getByText("Enrolled")).toBeTruthy();
  });

  it("stays open on a click on its own padding, or a drag that ends on the backdrop", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    show();

    const dialog = await askToLeave(user);
    vi.spyOn(dialog, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, right: 100, bottom: 100 } as DOMRect);
    fireEvent.pointerDown(dialog, { clientX: 50, clientY: 50 });
    fireEvent.click(dialog, { clientX: 50, clientY: 50 });
    fireEvent.pointerDown(dialog, { clientX: 50, clientY: 50 });
    fireEvent.click(dialog, { clientX: 150, clientY: 150 });

    expect(screen.getByRole("dialog")).toBe(dialog);
  });

  it("returns focus to Leave Program even when the click did not focus it", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    show();

    const dialog = await askToLeave(user);
    (document.activeElement as HTMLElement).blur();
    expect(document.activeElement).toBe(document.body);
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));

    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Leave Program" }));
  });

  it("cannot be dismissed while leaving is in flight", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    let settle!: () => void;
    api.optOutOfCredits.mockReturnValue(new Promise<void>((resolve) => (settle = resolve)));
    show();

    const dialog = await askToLeave(user);
    await confirmLeave(user, dialog);
    await waitFor(() => expect(api.optOutOfCredits).toHaveBeenCalledTimes(1));
    fireEvent.keyDown(dialog, { key: "Escape" });
    fireEvent(dialog, new Event("cancel", { cancelable: true }));
    fireEvent.pointerDown(dialog, { clientX: -10, clientY: -10 });
    fireEvent.click(dialog, { clientX: -10, clientY: -10 });
    showModal.mockClear();
    (dialog as HTMLDialogElement).open = false;
    fireEvent(dialog, new Event("close"));

    expect(showModal).toHaveBeenCalledTimes(1);
    expect((dialog as HTMLDialogElement).open).toBe(true);
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(document.documentElement.style.overflow).toBe("hidden");
    expect((within(dialog).getByRole("button", { name: "Close" }) as HTMLButtonElement).disabled).toBe(true);
    expect((within(dialog).getByRole("button", { name: "Cancel" }) as HTMLButtonElement).disabled).toBe(true);

    settle();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(document.documentElement.style.overflow).toBe("");
  });

  it("opts out on confirm and shows the status the server reads back", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValueOnce(enrolled).mockResolvedValue(optedOut);
    api.optOutOfCredits.mockResolvedValue(undefined);
    show();

    await confirmLeave(user, await askToLeave(user));

    await waitFor(() => expect(api.optOutOfCredits).toHaveBeenCalledWith(identity));
    expect(await screen.findByText("Opted Out")).toBeTruthy();
    expect(document.activeElement?.textContent).toContain("Opted Out");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("button", { name: "Leave Program" })).toBeNull();
  });

  it("keeps the member enrolled and explains a refused opt-out without the server's text", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    api.optOutOfCredits.mockRejectedValue(new CatalystError("Cannot unregister with credits claimed this week", "u", 400, true));
    show();

    await confirmLeave(user, await askToLeave(user));

    expect((await screen.findByRole("alert")).textContent).toContain("You can't leave this week because you've already claimed credits");
    expect(screen.getByText("Enrolled")).toBeTruthy();
    expect(api.fetchCreditsStatus).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("dialog", { name: "Leave Marketplace Credits?" })).toBeTruthy();
  });

  it("shows a generic retry message for any other failure, cleared by cancelling", async () => {
    const user = userEvent.setup();
    api.fetchCreditsStatus.mockResolvedValue(enrolled);
    api.optOutOfCredits.mockRejectedValue(new CatalystError("database exploded", "u", 500, true));
    show();

    await confirmLeave(user, await askToLeave(user));

    const alert = await screen.findByRole("alert");
    expect((alert).textContent).toContain("Couldn't leave the program right now. Please try again.");
    expect(alert.textContent).not.toContain("database exploded");

    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }));
    await askToLeave(user);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

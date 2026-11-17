// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const api = vi.hoisted(() => ({ fetchSubscription: vi.fn(), putSubscription: vi.fn(), putEmail: vi.fn() }));
vi.mock("@/data/lib/account/notifications", async (original) => ({
  ...(await original<typeof import("@/data/lib/account/notifications")>()),
  fetchSubscription: api.fetchSubscription,
  putSubscription: api.putSubscription,
  putEmail: api.putEmail,
}));

import type { SubscriptionDetails, SubscriptionResponse } from "@/data/lib/account/notifications";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { NotificationsSettings } from "./NotificationsSettings";

const SIGNER = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const identity: AuthIdentity = {
  signer: SIGNER,
  ephemeral: { address: "0xeph", privateKey: "0x01" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};
const details: SubscriptionDetails = {
  ignore_all_email: false,
  ignore_all_in_app: false,
  message_type: { item_sold: { email: true, in_app: true }, bid_received: { email: false, in_app: true } },
};
const confirmed: SubscriptionResponse = { address: SIGNER, email: "jane.doe@example.com", details };

let client: QueryClient;

/** The text a field's `aria-describedby` points at, as assistive technology reads it. */
function describedBy(element: HTMLElement): string {
  return (element.getAttribute("aria-describedby") ?? "")
    .split(/\s+/)
    .filter(Boolean)
    .map((id) => document.getElementById(id)?.textContent ?? "")
    .join(" ");
}

function show() {
  return render(
    <QueryClientProvider client={client}>
      <NotificationsSettings identity={identity} />
    </QueryClientProvider>,
  );
}

/** The groups render only once the subscription is read. */
async function loaded() {
  await screen.findByText("Marketplace");
}

async function openMarketplace(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByText("Marketplace"));
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
});
afterEach(() => {
  cleanup();
  client.clear();
  vi.resetAllMocks();
});

describe("NotificationsSettings", () => {
  describe("while the subscription loads", () => {
    it("shows a busy placeholder, and the load failure once the read fails", async () => {
      let fail!: (error: Error) => void;
      api.fetchSubscription.mockReturnValue(new Promise((_, reject) => (fail = reject)));
      const { container } = show();

      expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();
      fail(new Error("offline"));
      expect((await screen.findByRole("alert")).textContent).toContain("We couldn't load your notification settings");
      expect(screen.queryByRole("switch")).toBeNull();
      expect((screen.getByRole("textbox", { name: "Email address" }) as HTMLInputElement).disabled).toBe(true);
    });
  });

  describe("when the member has a confirmed email", () => {
    it("shows the address as confirmed, the master switch and each type's switch", async () => {
      const user = userEvent.setup();
      api.fetchSubscription.mockResolvedValue(confirmed);
      show();
      await loaded();

      expect((screen.getByRole("textbox", { name: "Email address" }) as HTMLInputElement).value).toBe("jane.doe@example.com");
      expect(screen.getByText("confirmed")).toBeTruthy();
      expect((screen.getByRole("switch", { name: "All email notifications" }) as HTMLInputElement).checked).toBe(true);
      await openMarketplace(user);
      expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLInputElement).checked).toBe(true);
      expect((screen.getByRole("switch", { name: "Bid Received" }) as HTMLInputElement).checked).toBe(false);
      expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    });

    it("saves a toggled type with the whole details and shows it on", async () => {
      const user = userEvent.setup();
      let server = confirmed;
      api.fetchSubscription.mockImplementation(async () => server);
      api.putSubscription.mockImplementation(async (_identity, next: SubscriptionDetails) => {
        server = { ...server, details: next };
      });
      show();
      await loaded();
      await openMarketplace(user);

      await user.click(screen.getByRole("switch", { name: "Bid Received" }));

      await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(1));
      const [signedBy, sent] = api.putSubscription.mock.calls[0]!;
      expect(signedBy).toBe(identity);
      expect(sent.message_type.bid_received).toEqual({ email: true, in_app: true });
      expect(sent.message_type.item_sold).toEqual({ email: true, in_app: true });
      await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
      expect((screen.getByRole("switch", { name: "Bid Received" }) as HTMLInputElement).checked).toBe(true);
      expect(screen.queryByRole("alert")).toBeNull();
    });

    it("shows the failure and the server's setting again when a save fails", async () => {
      const user = userEvent.setup();
      api.fetchSubscription.mockResolvedValue(confirmed);
      api.putSubscription.mockRejectedValue(new Error("500"));
      show();
      await loaded();
      await openMarketplace(user);

      await user.click(screen.getByRole("switch", { name: "Bid Received" }));

      expect((await screen.findByRole("alert")).textContent).toContain("We couldn't save your notification settings");
      expect((screen.getByRole("switch", { name: "Bid Received" }) as HTMLInputElement).checked).toBe(false);
      expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLInputElement).checked).toBe(true);
    });

    it("mutes every email from the master switch, keeping each type's own setting", async () => {
      const user = userEvent.setup();
      api.fetchSubscription.mockResolvedValue(confirmed);
      api.putSubscription.mockResolvedValue(undefined);
      show();
      await loaded();

      await user.click(screen.getByRole("switch", { name: "All email notifications" }));

      await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(1));
      expect(api.putSubscription.mock.calls[0]![1]).toEqual({ ...details, ignore_all_email: true });
    });
  });

  describe("when the master switch fails to save", () => {
    it("shows the failure and the master switch back on", async () => {
      const user = userEvent.setup();
      api.fetchSubscription.mockResolvedValue(confirmed);
      api.putSubscription.mockRejectedValue(new Error("500"));
      show();
      await loaded();

      await user.click(screen.getByRole("switch", { name: "All email notifications" }));

      expect((await screen.findByRole("alert")).textContent).toContain("We couldn't save your notification settings");
      expect((screen.getByRole("switch", { name: "All email notifications" }) as HTMLInputElement).checked).toBe(true);
    });
  });

  describe("when the member has no email", () => {
    it.each([
      ["missing", { address: SIGNER, details }],
      ["empty", { address: SIGNER, email: "", details }],
      ["null", { address: SIGNER, email: null, details }],
    ])("asks for one, with no master switch and the types locked (%s email)", async (_case, subscription) => {
      const user = userEvent.setup();
      api.fetchSubscription.mockResolvedValue(subscription);
      show();
      await loaded();

      expect(screen.getByText(/Enter your email, then check your inbox/)).toBeTruthy();
      expect((screen.getByRole("textbox", { name: "Email address" }) as HTMLInputElement).value).toBe("");
      expect((screen.getByRole("button", { name: "Submit" }) as HTMLButtonElement).disabled).toBe(true);
      expect(screen.queryByRole("switch", { name: "All email notifications" })).toBeNull();
      expect(screen.getByText("Confirm your email to choose which notifications you get.")).toBeTruthy();
      await openMarketplace(user);
      expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLButtonElement).disabled).toBe(true);
    });

    it("rejects a malformed address without asking the server", async () => {
      const user = userEvent.setup();
      api.fetchSubscription.mockResolvedValue({ address: SIGNER, details });
      show();
      await loaded();

      await user.type(screen.getByRole("textbox", { name: "Email address" }), "not-an-email");
      await user.click(screen.getByRole("button", { name: "Submit" }));

      expect(describedBy(screen.getByRole("textbox", { name: "Email address" }))).toBe("Invalid email");
      expect(api.putEmail).not.toHaveBeenCalled();
    });

    it("sends a valid address and shows it pending once the server has it", async () => {
      const user = userEvent.setup();
      api.fetchSubscription.mockResolvedValueOnce({ address: SIGNER, details });
      api.fetchSubscription.mockResolvedValue({ address: SIGNER, unconfirmedEmail: "jane.doe@example.com", details });
      api.putEmail.mockResolvedValue(undefined);
      show();
      await loaded();

      await user.type(screen.getByRole("textbox", { name: "Email address" }), "  jane.doe@example.com ");
      await user.click(screen.getByRole("button", { name: "Submit" }));

      await waitFor(() => expect(api.putEmail).toHaveBeenCalledWith(identity, "jane.doe@example.com"));
      expect(await screen.findByText("pending approval")).toBeTruthy();
      expect(screen.getByText("Confirmation email sent, check your inbox.")).toBeTruthy();
      expect((screen.getByRole("button", { name: "Resend" }) as HTMLButtonElement).disabled).toBe(false);
    });

    it("keeps the typed address and shows the failure when the server refuses it", async () => {
      const user = userEvent.setup();
      api.fetchSubscription.mockResolvedValue({ address: SIGNER, details });
      api.putEmail.mockRejectedValue(new Error("refused"));
      show();
      await loaded();

      const field = screen.getByRole("textbox", { name: "Email address" });
      await user.type(field, "jane.doe@example.com");
      await user.click(screen.getByRole("button", { name: "Submit" }));

      await waitFor(() => expect(describedBy(field)).toBe("We couldn't save your email. Please try again."));
      expect((field as HTMLInputElement).value).toBe("jane.doe@example.com");
      expect(api.fetchSubscription).toHaveBeenCalledTimes(1);

      await user.type(field, "x");
      expect(describedBy(field)).not.toMatch(/couldn't save/);
    });
  });

  describe("when a confirmed member edits the address", () => {
    it("enables Save only for a different address", async () => {
      const user = userEvent.setup();
      api.fetchSubscription.mockResolvedValue(confirmed);
      show();
      await loaded();

      const field = screen.getByRole("textbox", { name: "Email address" });
      const card = field.closest("section")!;
      await user.clear(field);
      await user.type(field, "jane.doe@example.com");
      expect((within(card).getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
      await user.type(field, "x");
      expect((within(card).getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(false);
    });
  });
});

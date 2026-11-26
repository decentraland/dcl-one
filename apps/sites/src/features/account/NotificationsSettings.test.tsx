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

type User = ReturnType<typeof userEvent.setup>;

let identity: AuthIdentity;
let details: SubscriptionDetails;
let confirmed: SubscriptionResponse;
let client: QueryClient;
let user: User;

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

async function openMarketplace(user: User) {
  await user.click(await screen.findByText("Marketplace"));
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  identity = {
    signer: SIGNER,
    ephemeral: { address: "0xeph", privateKey: "0x01" },
    expiration: "2999-01-01T00:00:00.000Z",
    authChain: [],
  };
  details = {
    ignore_all_email: false,
    ignore_all_in_app: false,
    message_type: { item_sold: { email: true, in_app: true }, bid_received: { email: false, in_app: true } },
  };
  confirmed = { address: SIGNER, email: "jane.doe@example.com", details };
  user = userEvent.setup();
});
afterEach(() => {
  cleanup();
  client.clear();
  vi.resetAllMocks();
});

describe("NotificationsSettings", () => {
  describe("when the subscription is loading", () => {
    let fail: (error: Error) => void;
    let container: HTMLElement;

    beforeEach(() => {
      fail = () => undefined;
      api.fetchSubscription.mockReturnValueOnce(new Promise((_, reject) => (fail = reject)));
      ({ container } = show());
    });

    it("should show a busy placeholder", () => {
      expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();
    });

    describe("and the read fails", () => {
      beforeEach(() => {
        fail(new Error("offline"));
      });

      it("should show the load failure", async () => {
        expect((await screen.findByRole("alert")).textContent).toContain("We couldn't load your notification settings");
      });

      it("should show no switches and lock the email field", async () => {
        await screen.findByRole("alert");
        expect(screen.queryByRole("switch")).toBeNull();
        expect((screen.getByRole("textbox", { name: "Email address" }) as HTMLInputElement).disabled).toBe(true);
      });

      describe("and Try again reads it", () => {
        beforeEach(async () => {
          await screen.findByRole("alert");
          api.fetchSubscription.mockResolvedValue(confirmed);
          await user.click(screen.getByRole("button", { name: "Try again" }));
          await loaded();
        });

        it("should clear the failure", () => {
          expect(screen.queryByRole("alert")).toBeNull();
        });

        it("should fill in the email address", () => {
          expect((screen.getByRole("textbox", { name: "Email address" }) as HTMLInputElement).value).toBe("jane.doe@example.com");
        });
      });
    });
  });

  describe("when the member has a confirmed email", () => {
    beforeEach(() => {
      api.fetchSubscription.mockResolvedValue(confirmed);
    });

    describe("and the page loads", () => {
      beforeEach(async () => {
        show();
        await loaded();
      });

      it("should show the address as confirmed", () => {
        expect((screen.getByRole("textbox", { name: "Email address" }) as HTMLInputElement).value).toBe("jane.doe@example.com");
        expect(screen.getByText("confirmed")).toBeTruthy();
      });

      it("should show the master switch on", () => {
        expect((screen.getByRole("switch", { name: "All email notifications" }) as HTMLInputElement).checked).toBe(true);
      });

      describe("and the Marketplace group is opened", () => {
        beforeEach(async () => {
          await openMarketplace(user);
        });

        it("should show each type's switch with its setting", () => {
          expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLInputElement).checked).toBe(true);
          expect((screen.getByRole("switch", { name: "Bid Received" }) as HTMLInputElement).checked).toBe(false);
        });

        it("should keep Save disabled", () => {
          expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
        });
      });

      describe("and the master switch is turned off", () => {
        beforeEach(async () => {
          api.putSubscription.mockResolvedValue(undefined);
          await user.click(screen.getByRole("switch", { name: "All email notifications" }));
          await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(1));
        });

        it("should mute every email, keeping each type's own setting", () => {
          expect(api.putSubscription.mock.calls[0]![1]).toEqual({ ...details, ignore_all_email: true });
        });
      });

      describe("and the master switch fails to save", () => {
        beforeEach(async () => {
          api.putSubscription.mockRejectedValue(new Error("500"));
          await user.click(screen.getByRole("switch", { name: "All email notifications" }));
        });

        it("should show the failure", async () => {
          expect((await screen.findByRole("alert")).textContent).toContain("We couldn't save your notification settings");
        });

        it("should show the master switch back on", async () => {
          await screen.findByRole("alert");
          expect((screen.getByRole("switch", { name: "All email notifications" }) as HTMLInputElement).checked).toBe(true);
        });
      });

      describe("and the address is retyped unchanged", () => {
        let field: HTMLElement;
        let card: HTMLElement;

        beforeEach(async () => {
          field = screen.getByRole("textbox", { name: "Email address" });
          card = field.closest("section")!;
          await user.clear(field);
          await user.type(field, "jane.doe@example.com");
        });

        it("should keep Save disabled", () => {
          expect((within(card).getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
        });

        describe("and it is changed to a different address", () => {
          beforeEach(async () => {
            await user.type(field, "x");
          });

          it("should enable Save", () => {
            expect((within(card).getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(false);
          });
        });
      });
    });

    describe("and a toggled type is saved", () => {
      let server: SubscriptionResponse;

      beforeEach(async () => {
        server = confirmed;
        api.fetchSubscription.mockImplementation(async () => server);
        api.putSubscription.mockImplementation(async (_identity, next: SubscriptionDetails) => {
          server = { ...server, details: next };
        });
        show();
        await loaded();
        await openMarketplace(user);

        await user.click(screen.getByRole("switch", { name: "Bid Received" }));
        await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(1));
      });

      it("should sign the save as the member", () => {
        const [signedBy] = api.putSubscription.mock.calls[0]!;
        expect(signedBy).toBe(identity);
      });

      it("should save the whole details with the type on", () => {
        const [, sent] = api.putSubscription.mock.calls[0]!;
        expect(sent.message_type.bid_received).toEqual({ email: true, in_app: true });
        expect(sent.message_type.item_sold).toEqual({ email: true, in_app: true });
      });

      it("should show the type on once read again, with no failure", async () => {
        await waitFor(() => expect(api.fetchSubscription).toHaveBeenCalledTimes(2));
        expect((screen.getByRole("switch", { name: "Bid Received" }) as HTMLInputElement).checked).toBe(true);
        expect(screen.queryByRole("alert")).toBeNull();
      });
    });

    describe("and a toggled type fails to save", () => {
      beforeEach(async () => {
        api.putSubscription.mockRejectedValue(new Error("500"));
        show();
        await loaded();
        await openMarketplace(user);

        await user.click(screen.getByRole("switch", { name: "Bid Received" }));
      });

      it("should show the failure", async () => {
        expect((await screen.findByRole("alert")).textContent).toContain("We couldn't save your notification settings");
      });

      it("should show the server's setting again", async () => {
        await screen.findByRole("alert");
        expect((screen.getByRole("switch", { name: "Bid Received" }) as HTMLInputElement).checked).toBe(false);
        expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLInputElement).checked).toBe(true);
      });
    });

    describe("and the member toggles quickly", () => {
      let writes: Array<() => void>;

      beforeEach(async () => {
        writes = [];
        api.putSubscription.mockImplementation(() => new Promise<void>((resolve) => writes.push(resolve)));
        show();
        await loaded();
        await openMarketplace(user);

        await user.click(screen.getByRole("switch", { name: "Bid Received" }));
        await user.click(screen.getByRole("switch", { name: "Royalties Earned" }));
        await waitFor(() => expect(writes).toHaveLength(1));
        writes[0]!();
        await waitFor(() => expect(api.putSubscription).toHaveBeenCalledTimes(2));
        writes[1]!();
      });

      it("should build the second write on the first, still pending", () => {
        const second = api.putSubscription.mock.calls[1]![1] as SubscriptionDetails;
        expect(second.message_type.bid_received?.email).toBe(true);
        expect(second.message_type.royalties_earned?.email).toBe(true);
      });
    });
  });

  describe("when email is muted", () => {
    beforeEach(async () => {
      api.fetchSubscription.mockResolvedValue({ ...confirmed, details: { ...details, ignore_all_email: true } });
      show();
      await loaded();
    });

    it("should show the master switch off", () => {
      expect((screen.getByRole("switch", { name: "All email notifications" }) as HTMLInputElement).checked).toBe(false);
    });

    it("should explain how to choose notifications", () => {
      expect(screen.getByText("Turn on email notifications to choose which ones you get.")).toBeTruthy();
    });

    describe("and the Marketplace group is opened", () => {
      beforeEach(async () => {
        await openMarketplace(user);
      });

      it("should lock the type switches, so a type change cannot unmute it", () => {
        expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLButtonElement).disabled).toBe(true);
      });
    });
  });

  describe("when an address change is pending", () => {
    beforeEach(async () => {
      api.fetchSubscription.mockResolvedValue({ ...confirmed, unconfirmedEmail: "jane.new@example.com" });
      show();
      await loaded();
    });

    it("should show the pending address without claiming an email was sent", () => {
      expect(screen.getByText("This email address is awaiting confirmation.")).toBeTruthy();
      expect(screen.queryByText("Confirmation email sent, check your inbox.")).toBeNull();
    });

    describe("and the resend fails", () => {
      beforeEach(async () => {
        api.putEmail.mockRejectedValueOnce(new Error("send failed"));
        await user.click(screen.getByRole("button", { name: "Resend" }));
        await screen.findByText("We couldn't save your email. Please try again.");
      });

      it("should keep the address pending and offer a retry without claiming delivery", () => {
        expect(screen.getByText("pending approval")).toBeTruthy();
        expect(screen.queryByText("Confirmation email sent, check your inbox.")).toBeNull();
        expect((screen.getByRole("button", { name: "Resend" }) as HTMLButtonElement).disabled).toBe(false);
      });

      describe("and the next resend succeeds", () => {
        beforeEach(async () => {
          api.putEmail.mockResolvedValueOnce(undefined);
          await user.click(screen.getByRole("button", { name: "Resend" }));
        });

        it("should confirm the send and clear the failure", async () => {
          expect(await screen.findByText("Confirmation email sent, check your inbox.")).toBeTruthy();
          expect(screen.queryByText("We couldn't save your email. Please try again.")).toBeNull();
        });
      });
    });

    it("should hide the master switch", () => {
      expect(screen.queryByRole("switch", { name: "All email notifications" })).toBeNull();
    });

    describe("and the Marketplace group is opened", () => {
      beforeEach(async () => {
        await openMarketplace(user);
      });

      it("should lock the types until it is confirmed", () => {
        expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLButtonElement).disabled).toBe(true);
      });
    });
  });

  describe("when the member has no email", () => {
    describe.each(["missing", "empty", "null"] as const)("and the email is %s", (emailCase) => {
      beforeEach(async () => {
        const subscription: Partial<SubscriptionResponse> & { email?: string | null } =
          emailCase === "missing"
            ? { address: SIGNER, details }
            : { address: SIGNER, email: emailCase === "empty" ? "" : null, details };
        api.fetchSubscription.mockResolvedValue(subscription);
        show();
        await loaded();
      });

      it("should ask for one", () => {
        expect(screen.getByText(/Enter your email, then check your inbox/)).toBeTruthy();
      });

      it("should show an empty field and keep Submit disabled", () => {
        expect((screen.getByRole("textbox", { name: "Email address" }) as HTMLInputElement).value).toBe("");
        expect((screen.getByRole("button", { name: "Submit" }) as HTMLButtonElement).disabled).toBe(true);
      });

      it("should show no master switch", () => {
        expect(screen.queryByRole("switch", { name: "All email notifications" })).toBeNull();
      });

      it("should explain that the email needs confirming", () => {
        expect(screen.getByText("Confirm your email to choose which notifications you get.")).toBeTruthy();
      });

      describe("and the Marketplace group is opened", () => {
        beforeEach(async () => {
          await openMarketplace(user);
        });

        it("should lock the types", () => {
          expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLButtonElement).disabled).toBe(true);
        });
      });
    });

    describe("and a malformed address is submitted", () => {
      beforeEach(async () => {
        api.fetchSubscription.mockResolvedValue({ address: SIGNER, details });
        show();
        await loaded();

        await user.type(screen.getByRole("textbox", { name: "Email address" }), "not-an-email");
        await user.click(screen.getByRole("button", { name: "Submit" }));
      });

      it("should reject it", () => {
        expect(describedBy(screen.getByRole("textbox", { name: "Email address" }))).toBe("Invalid email");
      });

      it("should not ask the server", () => {
        expect(api.putEmail).not.toHaveBeenCalled();
      });
    });

    describe("and a valid address is submitted", () => {
      beforeEach(async () => {
        api.fetchSubscription.mockResolvedValueOnce({ address: SIGNER, details });
        api.fetchSubscription.mockResolvedValue({ address: SIGNER, unconfirmedEmail: "jane.doe@example.com", details });
        api.putEmail.mockResolvedValue(undefined);
        show();
        await loaded();

        await user.type(screen.getByRole("textbox", { name: "Email address" }), "  jane.doe@example.com ");
        await user.click(screen.getByRole("button", { name: "Submit" }));
      });

      it("should send the trimmed address", async () => {
        await waitFor(() => expect(api.putEmail).toHaveBeenCalledWith(identity, "jane.doe@example.com"));
      });

      it("should show it pending once the server has it", async () => {
        expect(await screen.findByText("pending approval")).toBeTruthy();
      });

      it("should say the confirmation was sent and offer to resend it", async () => {
        expect(await screen.findByText("Confirmation email sent, check your inbox.")).toBeTruthy();
        expect((screen.getByRole("button", { name: "Resend" }) as HTMLButtonElement).disabled).toBe(false);
      });
    });

    describe("and the address is being saved", () => {
      let land: () => void;
      let field: HTMLInputElement;

      beforeEach(async () => {
        land = () => undefined;
        api.fetchSubscription.mockResolvedValue({ address: SIGNER, details });
        api.putEmail.mockReturnValue(new Promise<void>((resolve) => (land = resolve)));
        show();
        await loaded();

        field = screen.getByRole("textbox", { name: "Email address" }) as HTMLInputElement;
        await user.type(field, "jane.doe@example.com");
        await user.click(screen.getByRole("button", { name: "Submit" }));
      });

      it("should lock the field", async () => {
        await waitFor(() => expect(field.disabled).toBe(true));
      });

      describe("and the save lands", () => {
        beforeEach(async () => {
          await waitFor(() => expect(field.disabled).toBe(true));
          land();
        });

        it("should unlock the field", async () => {
          await waitFor(() => expect(field.disabled).toBe(false));
        });
      });
    });

    describe("and the server refuses the address", () => {
      let field: HTMLElement;

      beforeEach(async () => {
        api.fetchSubscription.mockResolvedValue({ address: SIGNER, details });
        api.putEmail.mockRejectedValue(new Error("refused"));
        show();
        await loaded();

        field = screen.getByRole("textbox", { name: "Email address" });
        await user.type(field, "jane.doe@example.com");
        await user.click(screen.getByRole("button", { name: "Submit" }));
        await waitFor(() => expect(describedBy(field)).toBe("We couldn't save your email. Please try again."));
      });

      it("should show the failure on the field", () => {
        expect(describedBy(field)).toBe("We couldn't save your email. Please try again.");
      });

      it("should keep the typed address", () => {
        expect((field as HTMLInputElement).value).toBe("jane.doe@example.com");
      });

      it("should not read the subscription again", () => {
        expect(api.fetchSubscription).toHaveBeenCalledTimes(1);
      });

      describe("and the member types again", () => {
        beforeEach(async () => {
          await user.type(field, "x");
        });

        it("should clear the failure", () => {
          expect(describedBy(field)).not.toMatch(/couldn't save/);
        });
      });
    });
  });
});

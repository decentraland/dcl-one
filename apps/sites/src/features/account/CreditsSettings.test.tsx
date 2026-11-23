// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

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

type User = ReturnType<typeof userEvent.setup>;
type CreditsStatus = { status: string; optedOutAt: string | null };

let identity: AuthIdentity;
let enrolled: CreditsStatus;
let optedOut: CreditsStatus;
let client: QueryClient;
let showModal: Mock<(this: HTMLDialogElement) => void>;
let closeDialog: Mock<(this: HTMLDialogElement) => void>;

function show() {
  return render(
    <QueryClientProvider client={client}>
      <CreditsSettings identity={identity} />
    </QueryClientProvider>,
  );
}

async function askToLeave(user: User) {
  await user.click(await screen.findByRole("button", { name: "Leave Program" }));
  return screen.getByRole("dialog", { name: "Leave Marketplace Credits?" });
}

/** The confirmation's own Leave Program, not the card's one behind it. */
function confirmLeave(user: User, dialog: HTMLElement) {
  return user.click(within(dialog).getByRole("button", { name: "Leave Program" }));
}

const nativeShowModal = HTMLDialogElement.prototype.showModal;
const nativeClose = HTMLDialogElement.prototype.close;

beforeEach(() => {
  identity = {
    signer: SIGNER,
    ephemeral: { address: "0xeph", privateKey: "0x01" },
    expiration: "2999-01-01T00:00:00.000Z",
    authChain: [],
  };
  enrolled = { status: "enrolled", optedOutAt: null };
  optedOut = { status: "opted_out", optedOutAt: "2026-11-16T00:00:00.000Z" };
  client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  // jsdom has no modal dialogs: opening and closing only toggle `open`, so the dialog's role shows.
  showModal = vi.fn(function (this: HTMLDialogElement) {
    this.open = true;
  });
  closeDialog = vi.fn(function (this: HTMLDialogElement) {
    this.open = false;
  });
  HTMLDialogElement.prototype.showModal = showModal;
  HTMLDialogElement.prototype.close = closeDialog;
});
afterEach(() => {
  cleanup();
  HTMLDialogElement.prototype.showModal = nativeShowModal;
  HTMLDialogElement.prototype.close = nativeClose;
  client.clear();
  vi.restoreAllMocks();
  vi.resetAllMocks();
});

describe("CreditsSettings", () => {
  let user: User;
  let dialog: HTMLElement;

  beforeEach(() => {
    user = userEvent.setup();
  });

  describe("when the member is enrolled", () => {
    beforeEach(() => {
      api.fetchCreditsStatus.mockResolvedValue(enrolled);
    });

    describe("and the page loads", () => {
      beforeEach(() => {
        show();
      });

      it("should show the enrolled status", async () => {
        expect(await screen.findByText("Enrolled")).toBeTruthy();
      });

      it("should link to the program's docs", async () => {
        await screen.findByText("Enrolled");
        expect((screen.getByRole("link", { name: "Learn more" })).getAttribute("href")).toContain("/docs/");
      });

      it("should offer an enabled way to leave", async () => {
        await screen.findByText("Enrolled");
        expect((screen.getByRole("button", { name: "Leave Program" }) as HTMLButtonElement).disabled).toBe(false);
      });
    });

    describe("and the member asks to leave", () => {
      beforeEach(async () => {
        show();
        dialog = await askToLeave(user);
      });

      it("should open a modal confirmation", () => {
        expect(showModal).toHaveBeenCalledTimes(1);
      });

      it("should lock the page's scroll", () => {
        expect(document.documentElement.style.overflow).toBe("hidden");
      });

      it("should describe the confirmation", () => {
        expect(screen.getByRole("dialog").getAttribute("aria-describedby")).toBeTruthy();
      });

      describe.each([
        ["Cancel", (dialog: HTMLElement, user: User) => user.click(within(dialog).getByRole("button", { name: "Cancel" }))],
        ["Close", (dialog: HTMLElement, user: User) => user.click(within(dialog).getByRole("button", { name: "Close" }))],
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
      ])("and it is dismissed with %s", (_how, dismiss) => {
        beforeEach(async () => {
          await dismiss(dialog, user);
        });

        it("should close the confirmation", () => {
          expect(screen.queryByRole("dialog")).toBeNull();
        });

        it("should return focus to Leave Program", () => {
          expect(document.activeElement).toBe(screen.getByRole("button", { name: "Leave Program" }));
        });

        it("should unlock the page's scroll", () => {
          expect(document.documentElement.style.overflow).toBe("");
        });

        it("should not opt out", () => {
          expect(api.optOutOfCredits).not.toHaveBeenCalled();
        });

        it("should keep the member enrolled", () => {
          expect(screen.getByText("Enrolled")).toBeTruthy();
        });
      });

      describe("and a click lands on its own padding, or a drag ends on the backdrop", () => {
        beforeEach(() => {
          vi.spyOn(dialog, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, right: 100, bottom: 100 } as DOMRect);
          fireEvent.pointerDown(dialog, { clientX: 50, clientY: 50 });
          fireEvent.click(dialog, { clientX: 50, clientY: 50 });
          fireEvent.pointerDown(dialog, { clientX: 50, clientY: 50 });
          fireEvent.click(dialog, { clientX: 150, clientY: 150 });
        });

        it("should stay open", () => {
          expect(screen.getByRole("dialog")).toBe(dialog);
        });
      });

      describe("and focus has left Leave Program", () => {
        beforeEach(() => {
          (document.activeElement as HTMLElement).blur();
        });

        it("should leave the body focused", () => {
          expect(document.activeElement).toBe(document.body);
        });

        describe("and Cancel is clicked", () => {
          beforeEach(async () => {
            await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
          });

          it("should return focus to Leave Program", () => {
            expect(document.activeElement).toBe(screen.getByRole("button", { name: "Leave Program" }));
          });
        });
      });
    });

    describe("and leaving is in flight", () => {
      let settle: () => void;

      beforeEach(async () => {
        settle = () => undefined;
        api.optOutOfCredits.mockReturnValueOnce(new Promise<void>((resolve) => (settle = resolve)));
        show();
        dialog = await askToLeave(user);
        await confirmLeave(user, dialog);
        await waitFor(() => expect(api.optOutOfCredits).toHaveBeenCalledTimes(1));
        fireEvent.keyDown(dialog, { key: "Escape" });
        fireEvent(dialog, new Event("cancel", { cancelable: true }));
        fireEvent.pointerDown(dialog, { clientX: -10, clientY: -10 });
        fireEvent.click(dialog, { clientX: -10, clientY: -10 });
        showModal.mockClear();
        (dialog as HTMLDialogElement).open = false;
        fireEvent(dialog, new Event("close"));
      });

      it("should reopen the modal after a forced close", () => {
        expect(showModal).toHaveBeenCalledTimes(1);
      });

      it("should keep the confirmation open", () => {
        expect((dialog as HTMLDialogElement).open).toBe(true);
        expect(screen.getByRole("dialog")).toBe(dialog);
      });

      it("should keep the page's scroll locked", () => {
        expect(document.documentElement.style.overflow).toBe("hidden");
      });

      it("should disable Close and Cancel", () => {
        expect((within(dialog).getByRole("button", { name: "Close" }) as HTMLButtonElement).disabled).toBe(true);
        expect((within(dialog).getByRole("button", { name: "Cancel" }) as HTMLButtonElement).disabled).toBe(true);
      });

      describe("and it settles", () => {
        beforeEach(() => {
          settle();
        });

        it("should close the confirmation and unlock the page's scroll", async () => {
          await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
          expect(document.documentElement.style.overflow).toBe("");
        });
      });
    });

    describe("and the member confirms leaving", () => {
      async function leave() {
        show();
        await confirmLeave(user, await askToLeave(user));
      }

      describe("and the server reads back the opt-out", () => {
        beforeEach(async () => {
          api.fetchCreditsStatus.mockResolvedValueOnce(enrolled).mockResolvedValue(optedOut);
          api.optOutOfCredits.mockResolvedValueOnce(undefined);
          await leave();
        });

        it("should opt the member out", async () => {
          await waitFor(() => expect(api.optOutOfCredits).toHaveBeenCalledWith(identity));
        });

        it("should show and focus the status the server reads back", async () => {
          expect(await screen.findByText("Opted Out")).toBeTruthy();
          expect(document.activeElement?.textContent).toContain("Opted Out");
        });

        it("should close the confirmation and offer no way to leave", async () => {
          await screen.findByText("Opted Out");
          expect(screen.queryByRole("dialog")).toBeNull();
          expect(screen.queryByRole("button", { name: "Leave Program" })).toBeNull();
        });
      });

      describe("and the read after leaving fails", () => {
        let alert: HTMLElement;

        beforeEach(async () => {
          api.fetchCreditsStatus.mockResolvedValueOnce(enrolled).mockRejectedValue(new Error("offline"));
          api.optOutOfCredits.mockResolvedValueOnce(undefined);
          await leave();
          alert = await screen.findByRole("alert");
        });

        it("should show the failure", () => {
          expect(alert.textContent).toContain("We couldn't load your credits status");
        });

        it("should focus the failure", async () => {
          await waitFor(() => expect(document.activeElement).toBe(alert));
        });

        it("should not show a stale Enrolled with Leave", () => {
          expect(screen.queryByText("Enrolled")).toBeNull();
          expect(screen.queryByRole("button", { name: "Leave Program" })).toBeNull();
        });

        it("should offer to try again", () => {
          expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
        });
      });

      describe("and the server refuses because credits were claimed this week", () => {
        beforeEach(async () => {
          api.optOutOfCredits.mockRejectedValueOnce(new CatalystError("Cannot unregister with credits claimed this week", "u", 400, true));
          await leave();
        });

        it("should explain the refusal without the server's text", async () => {
          expect((await screen.findByRole("alert")).textContent).toContain("You can't leave this week because you've already claimed credits");
        });

        it("should keep the member enrolled without reading the status again", async () => {
          await screen.findByRole("alert");
          expect(screen.getByText("Enrolled")).toBeTruthy();
          expect(api.fetchCreditsStatus).toHaveBeenCalledTimes(1);
        });

        it("should keep the confirmation open", async () => {
          await screen.findByRole("alert");
          expect(screen.getByRole("dialog", { name: "Leave Marketplace Credits?" })).toBeTruthy();
        });
      });

      describe("and the server fails for any other reason", () => {
        let alert: HTMLElement;

        beforeEach(async () => {
          api.optOutOfCredits.mockRejectedValueOnce(new CatalystError("database exploded", "u", 500, true));
          await leave();
          alert = await screen.findByRole("alert");
        });

        it("should show a generic retry message", () => {
          expect((alert).textContent).toContain("Couldn't leave the program right now. Please try again.");
        });

        it("should not show the server's text", () => {
          expect(alert.textContent).not.toContain("database exploded");
        });

        describe("and the member cancels and asks to leave again", () => {
          beforeEach(async () => {
            await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }));
            await askToLeave(user);
          });

          it("should clear the message", () => {
            expect(screen.queryByRole("alert")).toBeNull();
          });
        });
      });
    });
  });

  describe.each([
    ["opted_out", "2026-11-16T00:00:00.000Z", "Opted Out"],
    ["not_registered", null, "Not Registered"],
  ])("when the member's status is %s", (status, optedOutAt, label) => {
    beforeEach(() => {
      api.fetchCreditsStatus.mockResolvedValue({ status, optedOutAt });
      show();
    });

    it(`should show ${label}`, async () => {
      expect(await screen.findByText(label)).toBeTruthy();
    });

    it("should offer no way to leave", async () => {
      await screen.findByText(label);
      expect(screen.queryByRole("button", { name: "Leave Program" })).toBeNull();
    });
  });

  describe("when the status cannot be read", () => {
    describe("and the page loads", () => {
      beforeEach(() => {
        api.fetchCreditsStatus.mockRejectedValue(new Error("offline"));
        show();
      });

      it("should say so", async () => {
        expect((await screen.findByRole("alert")).textContent).toContain("We couldn't load your credits status");
      });

      it("should offer nothing to leave", async () => {
        await screen.findByRole("alert");
        expect(screen.queryByRole("button", { name: "Leave Program" })).toBeNull();
      });
    });

    describe("and Try again is clicked after the failed read", () => {
      beforeEach(async () => {
        api.fetchCreditsStatus.mockRejectedValueOnce(new Error("offline")).mockResolvedValue(enrolled);
        show();
        await user.click(await screen.findByRole("button", { name: "Try again" }));
      });

      it("should read the status again", async () => {
        expect(await screen.findByText("Enrolled")).toBeTruthy();
      });

      it("should clear the failure and offer a way to leave", async () => {
        await screen.findByText("Enrolled");
        expect(screen.queryByRole("alert")).toBeNull();
        expect(screen.getByRole("button", { name: "Leave Program" })).toBeTruthy();
      });
    });
  });
});

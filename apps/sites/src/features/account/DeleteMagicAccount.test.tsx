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

const auth = vi.hoisted(() => ({
  getIdentity: vi.fn(),
  getIdToken: vi.fn(),
  hasMagicSession: vi.fn(),
  isLoggedIn: vi.fn(),
  signedFetch: vi.fn(),
}));
vi.mock("@/data/lib/auth/magic-config", () => ({ magicApiKey: () => "test-key" }));
vi.mock("@/data/lib/auth/magic-session", () => ({
  hasMagicSession: auth.hasMagicSession,
  magicClient: async () => ({ user: { getIdToken: auth.getIdToken, isLoggedIn: auth.isLoggedIn } }),
}));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: auth.getIdentity }));
vi.mock("@/data/lib/auth/signer", () => ({ signedFetch: auth.signedFetch }));

import { ACCOUNT_COPY } from "./copy";
import { DeleteMagicAccount } from "./DeleteMagicAccount";

const ADDRESS = "0x1234567890123456789012345678901234567890";
const copy = ACCOUNT_COPY.magicDeletion;

describe("DeleteMagicAccount", () => {
  let client: QueryClient;
  let user: ReturnType<typeof userEvent.setup>;
  let onSignOut: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    client = new QueryClient();
    user = userEvent.setup();
    onSignOut = vi.fn();

  });

  afterEach(() => {
    cleanup();
    client.clear();
    vi.restoreAllMocks();
    vi.resetAllMocks();
  });

  function show() {
    render(
      <QueryClientProvider client={client}>
        <DeleteMagicAccount address={ADDRESS} onSignOut={onSignOut} />
      </QueryClientProvider>,
    );
  }

  describe("when a Magic session is active", () => {
    let dialog: HTMLElement;

    beforeEach(async () => {
      auth.hasMagicSession.mockReturnValueOnce(true);
      show();
      await user.click(await screen.findByRole("button", { name: copy.action }));
      dialog = screen.getByRole("dialog", { name: copy.title });
    });

    it("should let the user cancel before requesting a Magic token", async () => {
      await user.click(within(dialog).getByRole("button", { name: copy.cancel }));
      expect(auth.getIdToken).not.toHaveBeenCalled();
    });

    describe("and the user confirms", () => {
      beforeEach(() => {
        auth.getIdentity.mockReturnValueOnce({ signer: ADDRESS });
        auth.isLoggedIn.mockResolvedValueOnce(true);
        auth.getIdToken.mockResolvedValueOnce("test-token");
      });

      async function confirm() {
        await user.click(within(dialog).getByRole("button", { name: copy.action }));
      }

      describe("and Magic accepts the deletion", () => {
        beforeEach(() => {
          auth.signedFetch.mockResolvedValueOnce(new Response("{}"));
        });

        it("should sign the user out", async () => {
          await confirm();
          await waitFor(() => expect(onSignOut).toHaveBeenCalledOnce());
        });
      });

      for (const [status, message] of [[403, copy.forbidden], [429, copy.rateLimited], [502, copy.unknown]] as const) {
        describe(`and the relay responds with ${status}`, () => {
          let cancelBody: ReturnType<typeof vi.spyOn>;

          beforeEach(() => {
            const response = new Response("{}", { status });
            cancelBody = vi.spyOn(response.body!, "cancel");
            auth.signedFetch.mockResolvedValueOnce(response);
          });

          it("should report the error and release the response body", async () => {
            await confirm();
            expect((await within(dialog).findByRole("alert")).textContent).toBe(message);
            expect(cancelBody).toHaveBeenCalledOnce();
          });
        });
      }
    });
  });
});

// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

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
  signRequest: vi.fn(),
}));
vi.mock("@/data/lib/auth/magic-config", () => ({ magicApiKey: () => "test-key" }));
vi.mock("@/data/lib/auth/magic-session", () => ({
  hasMagicSession: auth.hasMagicSession,
  magicClient: async () => ({ user: { getIdToken: auth.getIdToken, isLoggedIn: auth.isLoggedIn } }),
}));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: auth.getIdentity }));
vi.mock("@/data/lib/auth/signer", () => ({ signRequest: auth.signRequest }));

import { ACCOUNT_COPY } from "./copy";
import { DeleteMagicAccount } from "./DeleteMagicAccount";

const ADDRESS = "0x1234567890123456789012345678901234567890";
const copy = ACCOUNT_COPY.magicDeletion;

describe("DeleteMagicAccount", () => {
  let fetchMock: Mock<typeof fetch>;
  let client: QueryClient;
  let user: ReturnType<typeof userEvent.setup>;
  let onSignOut: Mock<() => void>;

  beforeEach(() => {
    fetchMock = vi.fn<typeof fetch>();
    vi.stubGlobal("fetch", fetchMock);
    client = new QueryClient();
    user = userEvent.setup();
    onSignOut = vi.fn<() => void>();

  });

  afterEach(() => {
    cleanup();
    client.clear();
    vi.unstubAllGlobals();
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
        auth.signRequest.mockResolvedValue({ headers: { "x-identity-metadata": "signed" } });
      });

      async function confirm() {
        await user.click(within(dialog).getByRole("button", { name: copy.action }));
      }

      describe("and Magic accepts the deletion", () => {
        let response: Response;

        beforeEach(() => {
          response = new Response("{}");
          vi.spyOn(response.body!, "cancel");
          fetchMock.mockResolvedValueOnce(response);
        });

        it("should release the response body and sign the user out", async () => {
          await confirm();
          await waitFor(() => expect(onSignOut).toHaveBeenCalledOnce());
          expect(auth.signRequest).toHaveBeenCalledWith({ signer: ADDRESS }, "DELETE", "/auth-api/accounts", { didToken: "test-token" });
          expect(fetchMock).toHaveBeenCalledWith("/auth-api/accounts", { method: "DELETE", headers: { "x-identity-metadata": "signed" } });
          expect(response.body!.cancel).toHaveBeenCalledOnce();
        });
      });

      describe("and the connection drops during deletion", () => {
        beforeEach(() => {
          fetchMock.mockRejectedValueOnce(new TypeError("Failed to fetch"));
        });

        it("should explain the uncertain outcome and keep the user signed in", async () => {
          await confirm();
          expect((await within(dialog).findByRole("alert")).textContent).toContain(copy.unknown);
          expect(onSignOut).not.toHaveBeenCalled();
        });
      });

      describe("and signing fails before dispatch", () => {
        beforeEach(() => {
          auth.signRequest.mockRejectedValueOnce(new Error("Auth identity has expired"));
        });

        it("should surface the signing error before sending the request", async () => {
          await confirm();
          expect((await within(dialog).findByRole("alert")).textContent).toContain("Auth identity has expired");
          expect(fetchMock).not.toHaveBeenCalled();
        });
      });

      const failures = [
        { status: 403, error: "Origin not allowed", message: copy.originRefused, support: false },
        { status: 403, error: "Magic DID token is too old", message: copy.forbidden, support: false },
        { status: 403, error: "Magic DID token was already used", message: copy.reused, support: false },
        { status: 403, error: "Magic wallet does not match the request signer", message: copy.walletMismatch, support: false },
        { status: 403, error: "Account deletion requires a valid signed request", message: copy.signInAgain, support: false },
        { status: 429, error: "Account deletion is busy", message: copy.rateLimited, support: true },
        { status: 502, error: "The outcome of the Magic deletion request is unknown", message: copy.unknown, support: false },
        { status: 504, error: "Gateway Timeout", message: copy.unknown, support: false },
        { status: 503, error: "Account deletion is unavailable", message: copy.unavailable, support: true },
      ];
      for (const { status, error, message, support } of failures) {
        describe(`and the relay responds with ${status} "${error}"`, () => {
          let response: Response;

          beforeEach(() => {
            response = new Response(JSON.stringify({ error }), { status });
            fetchMock.mockResolvedValueOnce(response);
          });

          it(`should show its message${support ? " with a support link" : ""} and read the response body`, async () => {
            await confirm();
            const alert = await within(dialog).findByRole("alert");
            expect(alert.textContent).toContain(message);
            expect(within(alert).queryByRole("link", { name: copy.support })?.getAttribute("href") ?? null).toBe(
              support ? copy.supportHref : null,
            );
            expect(response.bodyUsed).toBe(true);
          });
        });
      }
    });
  });
});

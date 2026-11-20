// @vitest-environment jsdom
import { privateKeyToAccount } from "viem/accounts";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { buildEphemeralMessage } from "./identity";
import { beginShellBrowserSignIn, RELAY_PATH } from "./native-shell";
import { setIdentity } from "./session";

vi.mock("./session", () => ({ setIdentity: vi.fn() }));
vi.mock("./signin-kind", () => ({ forgetSignIn: vi.fn() }));

const shell = window as Window & {
  __DCL_SHELL_LAST_AUTH_URL?: string;
  __DCL_SHELL_TEST_SUPPRESS_NAV__?: boolean;
};
const fetchRelay = vi.fn<typeof fetch>();

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("fetch", fetchRelay);
  shell.__DCL_SHELL_TEST_SUPPRESS_NAV__ = true;
});
afterEach(() => {
  delete shell.__DCL_SHELL_TEST_SUPPRESS_NAV__;
  delete shell.__DCL_SHELL_LAST_AUTH_URL;
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.resetAllMocks();
});

describe("when desktop sign-in polls the relay", () => {
  it("should retry an invalid response and install only the approved identity", async () => {
    const account = privateKeyToAccount(`0x${"11".repeat(32)}`);
    const request = beginShellBrowserSignIn();
    await vi.dynamicImportSettled();
    const approval = new URL(shell.__DCL_SHELL_LAST_AUTH_URL!);
    const ephemeral = approval.searchParams.get("ephemeral")!;
    const expiration = new Date(approval.searchParams.get("expiration")!);
    const signature = await account.signMessage({ message: buildEphemeralMessage(ephemeral, expiration) });
    fetchRelay.mockResolvedValueOnce(new Response("invalid json")).mockResolvedValueOnce(Response.json({
      signer: account.address,
      signature,
    }));

    await vi.advanceTimersByTimeAsync(2_000);
    expect(setIdentity).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(request.identity).resolves.toMatchObject({ signer: account.address.toLowerCase() });
    expect(approval.pathname).toBe("/auth/native");
    expect(fetchRelay.mock.calls[0]?.[0]).toBe(`${RELAY_PATH}?id=${approval.searchParams.get("id")}`);
    expect(setIdentity).toHaveBeenCalledOnce();
  });

  it("should let go of the body of every answer that is not the approval", async () => {
    const waiting = new Response("not approved yet", { status: 404 });
    const cancel = vi.spyOn(waiting.body!, "cancel");
    const request = beginShellBrowserSignIn();
    await vi.dynamicImportSettled();
    fetchRelay.mockResolvedValueOnce(waiting);

    await vi.advanceTimersByTimeAsync(2_000);
    request.cancel();

    await expect(request.identity).resolves.toBeNull();
    expect(cancel).toHaveBeenCalledOnce();
    expect(setIdentity).not.toHaveBeenCalled();
  });

  it("should keep polling when letting go of a body fails", async () => {
    const waiting = new Response("not approved yet", { status: 503 });
    vi.spyOn(waiting.body!, "cancel").mockRejectedValue(new Error("stream locked"));
    const request = beginShellBrowserSignIn();
    await vi.dynamicImportSettled();
    fetchRelay.mockResolvedValueOnce(waiting).mockResolvedValue(new Response(null, { status: 404 }));

    await vi.advanceTimersByTimeAsync(4_000);
    request.cancel();

    await expect(request.identity).resolves.toBeNull();
    expect(fetchRelay).toHaveBeenCalledTimes(2);
  });

  it("should abort its request signal once the identity is installed", async () => {
    const account = privateKeyToAccount(`0x${"11".repeat(32)}`);
    const request = beginShellBrowserSignIn();
    await vi.dynamicImportSettled();
    const approval = new URL(shell.__DCL_SHELL_LAST_AUTH_URL!);
    const expiration = new Date(approval.searchParams.get("expiration")!);
    const signature = await account.signMessage({
      message: buildEphemeralMessage(approval.searchParams.get("ephemeral")!, expiration),
    });
    fetchRelay.mockResolvedValueOnce(Response.json({ signer: account.address, signature }));

    await vi.advanceTimersByTimeAsync(2_000);
    await request.identity;

    const signal = fetchRelay.mock.calls[0]?.[1]?.signal as AbortSignal;
    expect(signal.aborted).toBe(true);
  });

  it("should settle cancellation while waiting to poll", async () => {
    const request = beginShellBrowserSignIn();
    await vi.dynamicImportSettled();
    request.cancel();
    await expect(request.identity).resolves.toBeNull();
    expect(fetchRelay).not.toHaveBeenCalled();
    expect(setIdentity).not.toHaveBeenCalled();
  });

  it("should abort a pending request at the deadline", async () => {
    fetchRelay.mockImplementationOnce((_url, init) => new Promise((_resolve, reject) => {
      init?.signal?.addEventListener("abort", () => reject(init.signal?.reason), { once: true });
    }));
    const request = beginShellBrowserSignIn();
    await vi.dynamicImportSettled();
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    await expect(request.identity).resolves.toBeNull();
    expect(fetchRelay.mock.calls[0]?.[1]?.signal?.aborted).toBe(true);
    expect(setIdentity).not.toHaveBeenCalled();
  });
});

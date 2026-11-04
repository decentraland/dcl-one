// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getIdentity, setIdentity } from "@/data/lib/auth/session";
import type { AuthIdentity } from "@/data/lib/auth/types";

import MagicCallback from "./MagicCallback";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

const SIGNER = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const finish = vi.hoisted(() => vi.fn());
vi.mock("./magic", async (original) => ({ ...(await original<typeof import("./magic")>()), finishMagicSignIn: finish }));

const assign = vi.fn();
const replace = vi.fn();
function at(search: string) {
  vi.stubGlobal("location", { href: `http://localhost:3000/auth/callback${search}`, origin: "http://localhost:3000", search, assign, replace });
}
const state = (redirectTo: string) => btoa(JSON.stringify({ customData: JSON.stringify({ redirectTo }) }));
const IDENTITY: AuthIdentity = {
  signer: SIGNER,
  ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"12".repeat(32)}` },
  expiration: new Date(Date.now() + 30 * 86_400_000).toISOString(),
  authChain: [{ type: "SIGNER", payload: SIGNER, signature: "" }],
};

beforeEach(() => {
  localStorage.clear();
  setIdentity(null);
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("returning from Magic", () => {
  it("keeps the signed session and goes on to the page the reader signed in from", async () => {
    finish.mockResolvedValueOnce(IDENTITY);
    at(`?state=${encodeURIComponent(state("/shop/items?sortBy=newest"))}`);
    await act(async () => {
      render(<MagicCallback magicApiKey="pk_example123" />);
    });
    expect(finish).toHaveBeenCalledWith("pk_example123");
    expect(getIdentity()?.signer).toBe(SIGNER);
    expect(localStorage.getItem(`single-sign-on-${SIGNER}`)).not.toBeNull();
    expect(assign).toHaveBeenCalledWith("/shop/items?sortBy=newest");
  });

  it("never goes off-site, whatever the state carries", async () => {
    finish.mockResolvedValueOnce(IDENTITY);
    at(`?state=${encodeURIComponent(state("https://evil.example/steal"))}`);
    await act(async () => {
      render(<MagicCallback magicApiKey="pk_example123" />);
    });
    expect(assign).toHaveBeenCalledWith("/");
  });

  it("goes back to sign-in when the reader cancelled at the provider or the OAuth session was already used", async () => {
    at(`?error=access_denied&state=${encodeURIComponent(state("/shop"))}`);
    await act(async () => {
      render(<MagicCallback magicApiKey="pk_example123" />);
    });
    expect(finish).not.toHaveBeenCalled();
    expect(replace).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fshop");
    cleanup();

    finish.mockRejectedValueOnce(Object.assign(new Error("used"), { code: "MISSING_PKCE_METADATA" }));
    at(`?state=${encodeURIComponent(state("/shop"))}`);
    await act(async () => {
      render(<MagicCallback magicApiKey="pk_example123" />);
    });
    expect(replace).toHaveBeenCalledTimes(2);
  });

  it("shows the failure with Try Again, which returns to sign-in", async () => {
    finish.mockRejectedValueOnce(new Error("Magic said no"));
    at(`?state=${encodeURIComponent(state("/shop"))}`);
    await act(async () => {
      render(<MagicCallback magicApiKey="pk_example123" />);
    });
    expect(screen.getByTestId("connection-status").textContent).toBe("Something went wrong. Please try again.");
    expect(screen.getByTestId("connection-error-detail").textContent).toBe("Magic said no");
    expect(getIdentity()).toBeNull();
    act(() => screen.getByRole("button", { name: "Try Again" }).click());
    expect(replace).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fshop");
  });
});

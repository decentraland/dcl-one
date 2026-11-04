// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});
const magic = vi.hoisted(() => ({ key: "pk_example123", start: vi.fn(async () => {}) }));
vi.mock("@/data/lib/auth/magic-config", () => ({ magicApiKey: () => magic.key }));
vi.mock("../../auth/login/magic", () => ({ startMagicSignIn: magic.start }));
vi.mock("@/data/lib/auth/index", () => ({
  useAuth: () => ({
    isConnected: false,
    inAppAvailable: true,
    walletAvailable: false,
    detectedWallets: [],
    error: null,
    startSocialSignIn: vi.fn(),
  }),
}));

import SignInModalHost from "./SignInModalHost";
import { closeSignIn, openSignIn } from "./signin-store";

beforeEach(() => {
  vi.stubGlobal("location", { pathname: "/create/storage", search: "?step=edit", hash: "", origin: "http://localhost:3000", assign: vi.fn() });
});
afterEach(() => {
  cleanup();
  closeSignIn();
  vi.unstubAllGlobals();
  magic.start.mockClear();
  magic.key = "pk_example123";
});

async function openModal() {
  render(
    <MemoryRouter>
      <SignInModalHost />
    </MemoryRouter>,
  );
  await act(async () => {
    openSignIn({ inPlace: true });
  });
}

describe("the in-place sign-in modal", () => {
  it("signs in with Google through Magic, back to this page, as /auth/login does", async () => {
    await openModal();
    fireEvent.click(await screen.findByRole("button", { name: "Continue with Google" }));
    expect(magic.start).toHaveBeenCalledWith("pk_example123", "google", "/create/storage?step=edit");
  });

  it("offers no social option where Magic is not configured, rather than a different wallet", async () => {
    magic.key = "";
    await openModal();
    await screen.findByRole("dialog");
    expect(screen.queryByRole("button", { name: "Continue with Google" })).toBeNull();
  });
});

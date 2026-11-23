// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

import { installHostSignIn, openSignIn } from "./signin-store";

const assign = vi.fn();

beforeEach(() => {
  vi.stubGlobal("location", { pathname: "/shop/items", search: "?sortBy=newest", hash: "#grid", assign });
});
afterEach(() => {
  vi.unstubAllGlobals();
  assign.mockClear();
  delete (window as { __DCL_SHELL__?: unknown }).__DCL_SHELL__;
});

describe("when a visitor requests sign-in", () => {
  it("should return to the current path, query and hash", () => {
    openSignIn();
    expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fshop%2Fitems%3FsortBy%3Dnewest%23grid");
  });

  it("should preserve the return path supplied by the caller", () => {
    openSignIn({ redirectTo: "/shop/checkout" });
    expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fshop%2Fcheckout");
  });

  it("should ask the embedded Explorer host to handle sign-in", () => {
    const hostSignIn = vi.fn();
    const uninstall = installHostSignIn(hostSignIn);

    try {
      openSignIn();
      expect(hostSignIn).toHaveBeenCalledOnce();
      expect(assign).not.toHaveBeenCalled();
    } finally {
      uninstall();
    }
  });

  it("should send the desktop shell to the sign-in page", () => {
    (window as { __DCL_SHELL__?: unknown }).__DCL_SHELL__ = { shell: "tauri" };
    openSignIn({ redirectTo: "/create/storage?step=edit" });
    expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fcreate%2Fstorage%3Fstep%3Dedit");
  });
});

describe("when listening for browser sign-in", () => {
  let store: typeof import("./signin-store");
  let listener: Mock<(returnTo: string) => void>;
  let unsubscribe: () => void;

  beforeEach(async () => {
    store = await import("./signin-store");
    listener = vi.fn<(returnTo: string) => void>();
    unsubscribe = store.onBeforeBrowserSignIn(listener);
  });

  afterEach(() => {
    unsubscribe();
  });

  it("should pass the return path until the listener unsubscribes", () => {
    store.openSignIn({ redirectTo: "/report#evidence" });
    expect(listener).toHaveBeenCalledExactlyOnceWith("/report#evidence");
    unsubscribe();
    store.openSignIn({ redirectTo: "/report" });
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("should skip notifications for host sign-in", () => {
    const uninstall = store.installHostSignIn(vi.fn());
    store.openSignIn();
    uninstall();
    expect(listener).not.toHaveBeenCalled();
  });

  it("should keep the page when a listener declines the navigation", () => {
    const allow = store.onBeforeBrowserSignIn(() => false);
    try {
      store.openSignIn();
      expect(assign).not.toHaveBeenCalled();
    } finally {
      allow();
    }
  });
});

// @vitest-environment jsdom
import { cleanup, render, waitFor } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

import { INTERCOM_WIDGET_URL, showIntercom, useIntercom } from "./intercom";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const APP_ID = "app-example123";
const BOOT_SETTINGS = { app_id: APP_ID, alignment: "right", hide_default_launcher: true };
const READY_CALL = ["ready", expect.any(Function)];
const WALLET = "0x" + "ab".repeat(20);
const OTHER_WALLET = "0x" + "cd".repeat(20);

type IntercomWindow = Window & { Intercom?: (...args: unknown[]) => void; intercomSettings?: unknown };
const win = window as IntercomWindow;

function Widget({ appId = APP_ID, wallet }: { appId?: string; wallet?: string | null }) {
  useIntercom(appId, { alignment: "right" }, wallet);
  return null;
}

const scripts = () => Array.from(document.querySelectorAll<HTMLScriptElement>("script[data-intercom]"));

/** Plays the widget script: it defines window.Intercom, then fires load. */
function loadScript(intercom: (...args: unknown[]) => void) {
  win.Intercom = intercom;
  scripts()[0].dispatchEvent(new Event("load"));
}

let intercom: Mock<(...args: unknown[]) => void>;

beforeEach(() => {
  vi.spyOn(console, "error").mockImplementation(() => {});
  intercom = vi.fn();
});

afterEach(() => {
  cleanup();
  for (const script of scripts()) script.remove();
  delete win.Intercom;
  delete win.intercomSettings;
  vi.restoreAllMocks();
});

describe("useIntercom", () => {
  describe("when rendered on the server", () => {
    it("should not inject the widget", () => {
      renderToString(<Widget />);
      expect(scripts()).toHaveLength(0);
    });
  });

  describe("when mounted in the browser", () => {
    it("should inject the widget script for the app id", () => {
      render(<Widget />);
      expect(scripts().map((s) => s.src)).toEqual([`${INTERCOM_WIDGET_URL}${APP_ID}`]);
    });

    it("should set the widget settings", () => {
      render(<Widget />);
      expect(win.intercomSettings).toEqual({ alignment: "right", hide_default_launcher: true });
    });
  });

  describe("when the script loads", () => {
    describe("and a wallet is signed in", () => {
      it("should boot the widget with the app id, the alignment and the wallet", async () => {
        render(<Widget wallet={WALLET} />);
        loadScript(intercom);
        await waitFor(() =>
          expect(intercom.mock.calls).toEqual([READY_CALL, ["boot", { ...BOOT_SETTINGS, Wallet: WALLET }]]),
        );
      });
    });

    describe("and the messenger has not replaced Intercom's queue yet", () => {
      it("should queue the boot", async () => {
        render(<Widget wallet={WALLET} />);
        const queue = win.Intercom as unknown as { q: unknown[][] };
        scripts()[0].dispatchEvent(new Event("load"));
        await waitFor(() =>
          expect(queue.q).toEqual([READY_CALL, ["boot", { ...BOOT_SETTINGS, Wallet: WALLET }]]),
        );
      });
    });

    describe("and no one is signed in", () => {
      it("should leave the wallet out", async () => {
        render(<Widget wallet={null} />);
        loadScript(intercom);
        await waitFor(() => expect(intercom).toHaveBeenCalledWith("boot", BOOT_SETTINGS));
      });
    });
  });

  describe("when the page unmounts after the boot", () => {
    it("should shut the widget down", async () => {
      const { unmount } = render(<Widget />);
      loadScript(intercom);
      await waitFor(() => expect(intercom).toHaveBeenCalledWith("boot", expect.anything()));
      unmount();
      expect(intercom).toHaveBeenLastCalledWith("shutdown");
    });

    describe("and the visitor comes back to the page", () => {
      beforeEach(async () => {
        const first = render(<Widget />);
        loadScript(intercom);
        await waitFor(() => expect(intercom).toHaveBeenCalledWith("boot", expect.anything()));
        first.unmount();
        intercom.mockClear();
      });

      it("should boot again", async () => {
        render(<Widget />);
        await waitFor(() =>
          expect(intercom.mock.calls).toEqual([READY_CALL, ["boot", BOOT_SETTINGS]]),
        );
      });

      it("should reuse the injected script", async () => {
        render(<Widget />);
        await waitFor(() => expect(intercom).toHaveBeenCalledWith("boot", BOOT_SETTINGS));
        expect(scripts()).toHaveLength(1);
      });
    });
  });

  describe("when the wallet changes after the boot", () => {
    it("should restart the messenger for the new account", async () => {
      const view = render(<Widget wallet={WALLET} />);
      loadScript(intercom);
      await waitFor(() => expect(intercom).toHaveBeenCalledWith("boot", expect.objectContaining({ Wallet: WALLET })));
      intercom.mockClear();

      view.rerender(<Widget wallet={OTHER_WALLET} />);
      await waitFor(() =>
        expect(intercom.mock.calls).toEqual([
          ["shutdown"],
          READY_CALL,
          ["boot", { ...BOOT_SETTINGS, Wallet: OTHER_WALLET }],
        ]),
      );
    });
  });

  describe("when the app id is empty", () => {
    it("should load nothing", () => {
      render(<Widget appId="" />);
      expect(scripts()).toHaveLength(0);
    });
  });

  describe("when the script fails to load", () => {
    beforeEach(() => {
      render(<Widget />);
      scripts()[0].dispatchEvent(new Event("error"));
    });

    it("should log the failure", async () => {
      await waitFor(() => expect(console.error).toHaveBeenCalledWith("Could not render Intercom", "Failed to load the Intercom script"));
    });

    it("should drop the script and leave Intercom undefined", async () => {
      await waitFor(() => expect(console.error).toHaveBeenCalled());
      expect(scripts()).toHaveLength(0);
      expect(win.Intercom).toBeUndefined();
    });
  });
});

describe("showIntercom", () => {
  describe("when no widget was requested", () => {
    it("should open nothing", () => {
      expect(showIntercom()).toBe(false);
    });
  });

  describe("when the widget is loaded", () => {
    beforeEach(() => {
      win.Intercom = intercom;
    });

    it("should report that the chat opened", () => {
      expect(showIntercom()).toBe(true);
    });

    it("should open the chat through the widget", () => {
      showIntercom();
      expect(intercom).toHaveBeenCalledWith("show");
    });
  });
});

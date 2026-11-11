// @vitest-environment jsdom
import { cleanup, render, waitFor } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { INTERCOM_WIDGET_URL, showIntercom, useIntercom } from "./intercom";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const APP_ID = "app-example123";
const WALLET = "0x" + "ab".repeat(20);

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

beforeEach(() => {
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  cleanup();
  for (const script of scripts()) script.remove();
  delete win.Intercom;
  delete win.intercomSettings;
  vi.restoreAllMocks();
});

describe("useIntercom", () => {
  it("does not inject the widget during a server render", () => {
    renderToString(<Widget />);
    expect(scripts()).toHaveLength(0);
  });

  it("injects the widget script for the app id once mounted in the browser", () => {
    render(<Widget />);
    expect(scripts().map((s) => s.src)).toEqual([`${INTERCOM_WIDGET_URL}${APP_ID}`]);
    expect(win.intercomSettings).toEqual({ alignment: "right" });
  });

  it("boots the widget with the app id and the wallet once the script loads", async () => {
    const intercom = vi.fn();
    render(<Widget wallet={WALLET} />);
    loadScript(intercom);
    await waitFor(() => expect(intercom).toHaveBeenCalledWith("update", { Wallet: WALLET, app_id: APP_ID }));
    expect(intercom).toHaveBeenCalledWith("reattach_activator");
  });

  it("queues the boot calls while the messenger has not replaced Intercom's queue yet", async () => {
    render(<Widget wallet={WALLET} />);
    const queue = win.Intercom as unknown as { q: unknown[][] };
    scripts()[0].dispatchEvent(new Event("load"));
    await waitFor(() =>
      expect(queue.q).toEqual([["reattach_activator"], ["update", { Wallet: WALLET, app_id: APP_ID }]]),
    );
  });

  it("leaves the wallet out when no one is signed in", async () => {
    const intercom = vi.fn();
    render(<Widget wallet={null} />);
    loadScript(intercom);
    await waitFor(() => expect(intercom).toHaveBeenCalledWith("update", { app_id: APP_ID }));
  });

  it("shuts the widget down when the page unmounts", async () => {
    const intercom = vi.fn();
    const { unmount } = render(<Widget />);
    loadScript(intercom);
    await waitFor(() => expect(intercom).toHaveBeenCalledWith("update", { app_id: APP_ID }));
    unmount();
    expect(intercom).toHaveBeenLastCalledWith("shutdown");
  });

  it("loads nothing when the app id is empty", () => {
    render(<Widget appId="" />);
    expect(scripts()).toHaveLength(0);
  });

  it("drops the script and logs when it fails to load", async () => {
    render(<Widget />);
    scripts()[0].dispatchEvent(new Event("error"));
    await waitFor(() => expect(console.error).toHaveBeenCalledWith("Could not render Intercom", "Failed to load the Intercom script"));
    expect(scripts()).toHaveLength(0);
    expect(win.Intercom).toBeUndefined();
  });
});

describe("showIntercom", () => {
  it("opens nothing when no widget was requested", () => {
    expect(showIntercom()).toBe(false);
  });

  it("opens the chat through the loaded widget", () => {
    const intercom = vi.fn();
    win.Intercom = intercom;
    expect(showIntercom()).toBe(true);
    expect(intercom).toHaveBeenCalledWith("show");
  });
});

import { afterEach, describe, expect, test, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import CreatorHubChrome from "./CreatorHubChrome";
import { ChromeAuthContext, type ChromeAuth } from "../../web/frames/chrome-auth";
import { ChromeNavContext } from "../../web/frames/chrome-nav";

const ACCOUNT = "0x4d02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa7bf2";

function auth(over: Partial<ChromeAuth> = {}): ChromeAuth {
  return {
    signedIn: true,
    account: ACCOUNT,
    mana: "",
    name: "esteban",
    committee: false,
    onSignIn: vi.fn(),
    onSignOut: vi.fn(),
    ...over,
  };
}

afterEach(() => cleanup());

describe("creator hub account entry", () => {
  test("the rail account button opens the menu with Download and Log out", async () => {
    const a = auth();
    render(
      <ChromeAuthContext.Provider value={a}>
        <CreatorHubChrome active="home" />
      </ChromeAuthContext.Provider>,
    );
    const triggers = screen.getAllByRole("button", { name: "esteban 0x4d02\u{2026}7bf2" });
    expect(triggers.length).toBeGreaterThan(0);
    await userEvent.click(triggers[triggers.length - 1]!);
    expect(screen.getByRole("menuitem", { name: "Download" })).toHaveAttribute("href", "/create/settings");
    expect(screen.getByRole("menuitem", { name: "Switch account" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("menuitem", { name: "Log out" }));
    expect(a.onSignOut).toHaveBeenCalledTimes(1);
  });

  test("framed by the Explorer, Switch account opens its sign-in in one step and Log out is the Explorer's own", async () => {
    const a = auth({ onSignOut: undefined, onSwitchAccount: vi.fn() });
    render(
      <ChromeAuthContext.Provider value={a}>
        <CreatorHubChrome active="home" />
      </ChromeAuthContext.Provider>,
    );
    const triggers = screen.getAllByRole("button", { name: "esteban 0x4d02\u{2026}7bf2" });
    await userEvent.click(triggers[triggers.length - 1]!);
    expect(screen.queryByRole("menuitem", { name: "Log out" })).toBeNull();
    await userEvent.click(screen.getByRole("menuitem", { name: "Switch account" }));
    expect(a.onSwitchAccount).toHaveBeenCalledTimes(1);
    expect(a.onSignIn).not.toHaveBeenCalled();
  });

  test("an explicit onAccount handler wins over the menu, and a signed-out rail offers Sign in", async () => {
    const onAccount = vi.fn();
    render(
      <ChromeAuthContext.Provider value={auth()}>
        <CreatorHubChrome active="home" onAccount={onAccount} />
      </ChromeAuthContext.Provider>,
    );
    const triggers = screen.getAllByRole("button", { name: "esteban 0x4d02\u{2026}7bf2" });
    await userEvent.click(triggers[triggers.length - 1]!);
    expect(onAccount).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
    cleanup();

    const a = auth({ signedIn: false, account: "", name: "" });
    render(
      <ChromeAuthContext.Provider value={a}>
        <CreatorHubChrome active="home" />
      </ChromeAuthContext.Provider>,
    );
    await userEvent.click(screen.getAllByRole("button", { name: "Sign in" })[0]!);
    expect(a.onSignIn).toHaveBeenCalledTimes(1);
  });
});

test("one mobile toggle opens both global and Creator Hub links and closes after navigation or Escape", async () => {
    const onTab = vi.fn();
    const navigate = vi.fn(() => true);
    const { container } = render(<ChromeNavContext.Provider value={{ navigate }}><CreatorHubChrome active="collections" signedIn={false} onTab={onTab} /></ChromeNavContext.Provider>);
    const toggle = container.querySelector<HTMLButtonElement>(".dtb__burger")!;
    toggle.style.display = "inline-flex";
    const panel = document.getElementById(toggle.getAttribute("aria-controls")!)!;
    panel.style.display = "block";
    expect(screen.queryByRole("button", { name: "Open menu" })).toBeNull();
    expect(container.querySelector(".ch__brand img")).toBeNull();
    expect(container.querySelector('a[href="/create/activity"]')).toBeNull();
    await userEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    const menu = document.getElementById(toggle.getAttribute("aria-controls")!)!;
    const creatorNav = within(menu).getByRole("navigation", { name: "Creator Hub sections" });
    expect(within(creatorNav).queryByRole("link", { name: /^Activity/ })).toBeNull();
    const collections = within(creatorNav).getByRole("link", { name: "Collections" });
    expect(collections).toHaveAttribute("aria-current", "page");
    await userEvent.click(collections);
    expect(navigate).toHaveBeenCalledWith("/create/wearables");
    expect(onTab).not.toHaveBeenCalled();
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(toggle);
    await userEvent.keyboard("{Escape}");
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveFocus();
  });

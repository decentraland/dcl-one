import { afterEach, describe, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import DclTopBar from "./DclTopBar";
import { ChromeAuthContext, type ChromeAuth } from "@dcl/one-ui/components/AppChrome/chrome-auth";

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

function renderBar(a: ChromeAuth) {
  return render(
    <ChromeAuthContext.Provider value={a}>
      <DclTopBar variant="sites" />
    </ChromeAuthContext.Provider>,
  );
}

const menu = () => screen.queryByRole("menu", { name: "My account" });
const openMenu = () => userEvent.click(screen.getByRole("button", { name: "My account" }));

afterEach(() => cleanup());

describe("top bar profile menu", () => {
  test("the avatar opens a menu with the account; Escape closes it; Switch account signs out then back in; Log out signs out and closes", async () => {
    const a = auth();
    renderBar(a);
    expect(menu()).toBeNull();

    await openMenu();
    expect(menu()).toHaveTextContent("esteban");
    expect(menu()).toHaveTextContent("0x4d02\u{2026}7bf2");
    expect(screen.getByRole("menuitem", { name: "My assets" })).toHaveAttribute("href", "/shop/my-items");
    await userEvent.keyboard("{Escape}");
    expect(menu()).toBeNull();

    await openMenu();
    await userEvent.click(screen.getByRole("menuitem", { name: "Switch account" }));
    expect(a.onSignOut).toHaveBeenCalledTimes(1);
    expect(a.onSignIn).toHaveBeenCalledTimes(1);

    await openMenu();
    await userEvent.click(screen.getByRole("menuitem", { name: "Log out" }));
    expect(a.onSignOut).toHaveBeenCalledTimes(2);
    expect(menu()).toBeNull();
  });

  test("signed out shows Sign in instead of the avatar, and Discover lists Events and Places", async () => {
    const a = auth({ signedIn: false, account: "", name: "" });
    renderBar(a);
    expect(screen.queryByRole("button", { name: "My account" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(a.onSignIn).toHaveBeenCalledTimes(1);

    await userEvent.hover(document.querySelectorAll(".dtb__dd")[0]!);
    expect(screen.getAllByRole("menuitem", { name: "Events", hidden: true })[0]).toHaveAttribute("href", "/events");
    expect(screen.getAllByRole("menuitem", { name: "Places", hidden: true })[0]).toHaveAttribute("href", "/places");
  });
});

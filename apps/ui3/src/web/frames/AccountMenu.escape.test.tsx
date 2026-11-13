import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import AccountMenu from "./AccountMenu";

describe("when AccountMenu shares keyboard dismissal with an outer dialog", () => {
  let user: ReturnType<typeof userEvent.setup>;
  let outerEscape: ReturnType<typeof vi.fn<() => void>>;
  let onDocumentKey: (event: KeyboardEvent) => void;
  beforeEach(() => {
    user = userEvent.setup();
    outerEscape = vi.fn();
    onDocumentKey = (event) => {
      if (event.key === "Escape" && !event.defaultPrevented) outerEscape();
    };
    document.addEventListener("keydown", onDocumentKey);
  });
  afterEach(() => {
    document.removeEventListener("keydown", onDocumentKey);
    vi.restoreAllMocks();
  });

  it.each(["trigger", "item"])(
    "should consume only the child Escape from the focused %s",
    async (from) => {
      render(
        <AccountMenu links={[{ label: "Profile", href: "/profile" }]}>
          Account
        </AccountMenu>,
      );
      const trigger = screen.getByRole("button", { name: "My account" });
      await user.click(trigger);
      if (from === "item") await user.tab();
      await user.keyboard("{Escape}");
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
      expect(trigger).toHaveFocus();
      expect(outerEscape).not.toHaveBeenCalled();
      await user.keyboard("{Escape}");
      expect(outerEscape).toHaveBeenCalledOnce();
    },
  );

  it("should close after Tab leaves its root without trapping or restoring focus", async () => {
    render(
      <>
        <AccountMenu links={[{ label: "Profile", href: "/profile" }]}>
          Account
        </AccountMenu>
        <button>After menu</button>
      </>,
    );
    await user.click(screen.getByRole("button", { name: "My account" }));
    await user.tab();
    expect(screen.getByRole("menuitem", { name: "Profile" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "After menu" })).toHaveFocus();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("should close on outside click without stealing the destination focus", async () => {
    render(
      <>
        <AccountMenu>Account</AccountMenu>
        <button>Outside</button>
      </>,
    );
    await user.click(screen.getByRole("button", { name: "My account" }));
    await user.click(screen.getByRole("button", { name: "Outside" }));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Outside" })).toHaveFocus();
  });

  it("should retain the menu for null-relatedTarget pointer blur before item activation", async () => {
    const navigate = vi.fn(() => true);
    render(
      <AccountMenu
        links={[{ label: "Profile", href: "/profile" }]}
        onNavigate={navigate}
      >
        Account
      </AccountMenu>,
    );
    const trigger = screen.getByRole("button", { name: "My account" });
    await user.click(trigger);
    fireEvent.blur(trigger, { relatedTarget: null });
    expect(screen.queryByRole("menu")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("menuitem", { name: "Profile" }));
    expect(navigate).toHaveBeenCalledWith("/profile");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(outerEscape).not.toHaveBeenCalled();
  });

  it("should retain app navigation without restoring the account trigger", async () => {
    const navigate = vi.fn();
    render(
      <AccountMenu
        onNavigate={navigate}
        links={[{ label: "Profile", href: "/profile" }]}
      >
        Account
      </AccountMenu>,
    );
    const trigger = screen.getByRole("button", { name: "My account" });
    await user.click(trigger);
    await user.click(screen.getByRole("menuitem", { name: "Profile" }));
    expect(navigate).toHaveBeenCalledWith("/profile");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(trigger).not.toHaveFocus();
  });

  it.each(["switch", "signOut"])(
    "should retain the %s callback without restoring focus",
    async (action) => {
      const callback = vi.fn();
      render(
        <AccountMenu
          onSwitchAccount={action === "switch" ? callback : undefined}
          onSignOut={action === "signOut" ? callback : undefined}
        >
          Account
        </AccountMenu>,
      );
      const trigger = screen.getByRole("button", { name: "My account" });
      await user.click(trigger);
      await user.click(
        screen.getByRole("menuitem", {
          name: action === "switch" ? "Switch account" : "Log out",
        }),
      );
      expect(callback).toHaveBeenCalledOnce();
      expect(trigger).not.toHaveFocus();
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    },
  );
  it("should keep Escape ownership after clicking the menu padding without canceling pointer defaults", async () => {
    render(<AccountMenu>Account</AccountMenu>);
    const trigger = screen.getByRole("button", { name: "My account" });
    await user.click(trigger);
    const menu = screen.getByRole("menu");
    const pointer = new MouseEvent("mousedown", {
      bubbles: true,
      cancelable: true,
    });
    menu.dispatchEvent(pointer);
    expect(pointer.defaultPrevented).toBe(false);
    await user.click(menu);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
    expect(outerEscape).not.toHaveBeenCalled();
  });

  it.each(["Ada", "0x1234…abcd"])(
    "should retain keyboard ownership after clicking static account text %s",
    async (label) => {
      render(
        <AccountMenu name="Ada" account="0x1234567890abcd">
          Account
        </AccountMenu>,
      );
      const trigger = screen.getByRole("button", { name: "My account" });
      await user.click(trigger);
      await user.click(screen.getByText(label));
      expect(trigger).toHaveFocus();
      expect(screen.queryByRole("menu")).toBeInTheDocument();
      await user.keyboard("{Escape}");
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
      expect(trigger).toHaveFocus();
      expect(outerEscape).not.toHaveBeenCalled();
    },
  );

  it("should focus the trigger on pointer opening when the browser does not focus clicked buttons", async () => {
    render(<AccountMenu>Account</AccountMenu>);
    const trigger = screen.getByRole("button", { name: "My account" });
    expect(document.activeElement).not.toBe(trigger);
    fireEvent.click(trigger);
    expect(trigger).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(outerEscape).not.toHaveBeenCalled();
  });
});

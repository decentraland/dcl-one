// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

import { ProfileMoreActions } from "./ProfileMoreActions";

afterEach(cleanup);

describe("ProfileMoreActions", () => {
  it("opens from the keyboard, skips disabled items and returns focus on Escape", async () => {
    const user = userEvent.setup();
    render(<ProfileMoreActions actions={[
      { label: "Block", icon: null, onSelect: vi.fn() },
      { label: "Unavailable", icon: null, disabled: true, onSelect: vi.fn() },
      { label: "Share", icon: null, onSelect: vi.fn() },
    ]} />);
    const trigger = screen.getByRole("button", { name: "More actions" });
    await user.tab();
    expect(document.activeElement).toBe(trigger);

    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Block" }));
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Share" }));
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Block" }));
    await user.keyboard("{ArrowUp}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Share" }));

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("closes an open menu on an outside press", async () => {
    const user = userEvent.setup();
    render(<ProfileMoreActions actions={[{ label: "Share", icon: null, onSelect: vi.fn() }]} />);
    await user.click(screen.getByRole("button", { name: "More actions" }));
    expect(screen.getByRole("menu")).toBeTruthy();

    await user.click(document.body);

    expect(screen.queryByRole("menu")).toBeNull();
  });
});

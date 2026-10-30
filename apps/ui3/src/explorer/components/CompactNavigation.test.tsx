import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CompactNavigation from "./CompactNavigation";

test("the compact picker supports keyboard selection, Escape and focus restoration", async () => {
  const user = userEvent.setup();
  const select = vi.fn();
  render(<CompactNavigation label="Communities" name="Switch section" items={[
    { kind: "button", label: "Events" },
    { kind: "button", label: "Unavailable", disabled: true },
    { kind: "button", label: "Marketplace", onClick: select },
  ]} />);
  const trigger = screen.getByRole("button", { name: "Switch section" });
  trigger.focus();
  await user.keyboard("{ArrowDown}");
  expect(screen.getByRole("menuitem", { name: "Events" })).toHaveFocus();
  await user.keyboard("{ArrowDown}{Enter}");
  expect(select).toHaveBeenCalledOnce();
  expect(screen.queryByRole("menu")).toBeNull();
  expect(trigger).toHaveFocus();
  await user.keyboard("{Enter}{End}");
  expect(screen.getByRole("menuitem", { name: "Marketplace" })).toHaveFocus();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("menu")).toBeNull();
  expect(trigger).toHaveFocus();
});

test("leaving the menu with Tab or a pointer dismisses it without trapping focus", async () => {
  const user = userEvent.setup();
  render(<><CompactNavigation label="Home" name="Social navigation" items={[
    { kind: "button", label: "Friends" },
  ]} /><button>Search</button></>);
  const trigger = screen.getByRole("button", { name: "Social navigation" });
  await user.click(trigger);
  await user.tab();
  expect(screen.getByRole("button", { name: "Search" })).toHaveFocus();
  expect(screen.queryByRole("menu")).toBeNull();
  await user.click(trigger);
  await user.click(screen.getByRole("button", { name: "Search" }));
  expect(screen.queryByRole("menu")).toBeNull();
});

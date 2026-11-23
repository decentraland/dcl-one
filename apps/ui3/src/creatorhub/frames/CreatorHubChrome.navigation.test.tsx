import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ChromeNavContext } from "@dcl/one-ui/components/AppChrome/chrome-nav";
import CreatorHubChrome from "./CreatorHubChrome";

afterEach(cleanup);

it("routes creator navigation through the existing app navigator", () => {
  const navigate = vi.fn(() => true);
  render(<ChromeNavContext value={{ navigate }}>
    <CreatorHubChrome active="collections" />
  </ChromeNavContext>);
  expect(fireEvent.click(screen.getByRole("link", { name: "Collections" }))).toBe(false);
  expect(navigate).toHaveBeenLastCalledWith("/create/collections");
});

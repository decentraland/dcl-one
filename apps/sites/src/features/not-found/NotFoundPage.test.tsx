// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, expect, it, vi } from "vitest";

import NotFoundPage from "./NotFoundPage";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));
vi.mock("@ui/shop/discovery/components/animated-background/AnimatedBackground", () => ({ default: () => null }));

afterEach(cleanup);

it("tells the visitor the page is gone and links home and to What's On", () => {
  render(
    <MemoryRouter>
      <NotFoundPage />
    </MemoryRouter>,
  );
  expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Oops! This page has left the world.");
  expect(screen.getByRole("link", { name: "Decentraland Home" }).getAttribute("href")).toBe("/");
  expect(screen.getByRole("link", { name: "See Events" }).getAttribute("href")).toBe("/whats-on");
  expect(screen.getByRole("main").getAttribute("data-status")).toBe("404");
});

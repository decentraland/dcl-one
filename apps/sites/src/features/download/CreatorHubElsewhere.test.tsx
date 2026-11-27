// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";

import CreatorHubElsewhere from "./CreatorHubElsewhere";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));

const writeText = vi.fn(async () => undefined);

function withNavigator(share: (() => Promise<void>) | undefined) {
  Object.defineProperty(navigator, "share", { value: share, configurable: true });
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it("copies the link when the share sheet is refused for another reason than a dismissal", async () => {
  withNavigator(vi.fn(async () => Promise.reject(new DOMException("no gesture", "NotAllowedError"))));
  render(<CreatorHubElsewhere sid="sid-1" />);

  await userEvent.click(screen.getByRole("button", { name: "Share link" }));

  expect(writeText).toHaveBeenCalledOnce();
  expect(await screen.findByRole("button", { name: "Link copied" })).toBeTruthy();
  expect(track).toHaveBeenCalledWith("lp_creatorhub_link_shared", { method: "copy" }, expect.anything());
});

it("does nothing more when the visitor dismisses the share sheet", async () => {
  withNavigator(vi.fn(async () => Promise.reject(new DOMException("dismissed", "AbortError"))));
  render(<CreatorHubElsewhere sid="sid-1" />);

  await userEvent.click(screen.getByRole("button", { name: "Share link" }));

  expect(writeText).not.toHaveBeenCalled();
  expect(track).not.toHaveBeenCalled();
});

it("links to the Creator Docs and reports the click", async () => {
  withNavigator(undefined);
  render(<CreatorHubElsewhere sid="sid-1" />);
  const docs = screen.getByRole("link", { name: "Explore the Creator Docs" });
  expect(docs.getAttribute("href")).toMatch(/creator/);
  docs.addEventListener("click", (event) => event.preventDefault());

  await userEvent.click(docs);

  expect(track).toHaveBeenCalledWith("lp_creatorhub_docs_clicked", {}, expect.objectContaining({ sid: "sid-1" }));
});

it("leaves the button as it was when the link cannot be copied", async () => {
  withNavigator(undefined);
  writeText.mockRejectedValueOnce(new Error("denied"));
  render(<CreatorHubElsewhere sid="sid-1" />);
  const share = screen.getByRole("button", { name: "Share link" });
  expect(share.getAttribute("type")).toBe("button");

  await userEvent.click(share);

  expect(writeText).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Share link" })).toBeTruthy();
  expect(track).not.toHaveBeenCalled();
});

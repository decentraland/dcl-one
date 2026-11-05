// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { loadWorldScenes, type WorldScene } from "@/data/lib/catalyst/places/world-scenes";
import WorldScenes from "./WorldScenes";

vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });
vi.mock("@/data/lib/catalyst/places/world-scenes", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/places/world-scenes")>(), loadWorldScenes: vi.fn(),
}));

describe("when browsing scenes in a world", () => {
  let onVisit: ReturnType<typeof vi.fn<(placeId: string, launchUrl: string) => void>>;
  let scenes: WorldScene[];
  let user: ReturnType<typeof userEvent.setup>;
  let view: ReturnType<typeof render>;
  beforeEach(async () => {
    scenes = [{ id: "gallery", title: "Gallery", base_position: "4,-5", world: true, world_name: "sample.dcl.eth" }];
    onVisit = vi.fn<(placeId: string, launchUrl: string) => void>();
    user = userEvent.setup();
    vi.mocked(loadWorldScenes).mockResolvedValue({ scenes, total: 21 });
    view = render(<MemoryRouter><WorldScenes worldName="sample.dcl.eth" selectedId="gallery" onVisit={onVisit} /></MemoryRouter>);
    await screen.findByRole("link", { name: "Gallery" });
  });
  afterEach(() => { cleanup(); vi.resetAllMocks(); });

  it("should expose scene details and coordinate-aware world travel", () => {
    expect(screen.getByRole("link", { name: "Gallery" }).getAttribute("href")).toBe("/places/gallery");
    expect(screen.getByRole("link", { name: "Visit Gallery" }).getAttribute("href")).toBe("/play/?realm=sample.dcl.eth&position=4%2C-5");
  });

  describe("and a visitor activates a scene's travel link", () => {
    beforeEach(() => { screen.getByRole("link", { name: "Visit Gallery" }).addEventListener("click", (event) => event.preventDefault(), { once: true }); });
    it("should report the selected scene and actual launch URL for handoff telemetry", async () => {
      await user.click(screen.getByRole("link", { name: "Visit Gallery" }));
      expect(onVisit).toHaveBeenCalledWith("gallery", "/play/?realm=sample.dcl.eth&position=4%2C-5");
    });
  });

  describe("and a later page fails", () => {
    beforeEach(async () => {
      vi.mocked(loadWorldScenes).mockRejectedValueOnce(new Error("Unavailable"));
      await user.click(screen.getByRole("button", { name: "Next scenes" }));
      await screen.findByRole("alert");
    });
    it("should retry the same offset and restore navigation", async () => {
      await user.click(screen.getByRole("button", { name: "Retry scenes" }));
      await screen.findByRole("link", { name: "Gallery" });
      expect(loadWorldScenes).toHaveBeenLastCalledWith("sample.dcl.eth", 20, expect.anything());
      expect(screen.getByRole("button", { name: "Previous scenes" }).hasAttribute("disabled")).toBe(false);
      expect(screen.getByRole("button", { name: "Next scenes" }).hasAttribute("disabled")).toBe(true);
    });
  });

  describe("and another world opens after paging", () => {
    beforeEach(async () => {
      await user.click(screen.getByRole("button", { name: "Next scenes" }));
      await screen.findByRole("link", { name: "Gallery" });
      vi.mocked(loadWorldScenes).mockClear();
      view.rerender(<MemoryRouter><WorldScenes worldName="other.dcl.eth" selectedId="other" /></MemoryRouter>);
    });
    it("should start the new world at its first page", async () => {
      await waitFor(() => expect(loadWorldScenes).toHaveBeenCalledWith("other.dcl.eth", 0, expect.anything()));
      expect(loadWorldScenes).not.toHaveBeenCalledWith("other.dcl.eth", 20, expect.anything());
    });
  });
});

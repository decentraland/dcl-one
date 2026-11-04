// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { searchFeaturedAssets, type FeaturedAsset } from "@/data/lib/catalyst/landings/hangout-options";
import FeaturedAssetPicker from "./FeaturedAssetPicker";

vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true, $RefreshSig$: () => (type: unknown) => type }); });
vi.mock("@/data/lib/catalyst/landings/hangout-options", () => ({ searchFeaturedAssets: vi.fn() }));

describe("when searching featured assets", () => {
  let user: ReturnType<typeof userEvent.setup>;
  let view: ReturnType<typeof render>;
  let asset: FeaturedAsset;
  let onChange: ReturnType<typeof vi.fn<(urn: string) => void>>;
  beforeEach(() => {
    user = userEvent.setup();
    asset = { urn: "urn:decentraland:matic:collections-v2:0x1111111111111111111111111111111111111111:2", name: "Music wearable", thumbnail: null, kind: "item" };
    onChange = vi.fn();
    vi.mocked(searchFeaturedAssets).mockResolvedValue([asset]);
    view = render(<FeaturedAssetPicker value="" onChange={onChange} />);
  });
  afterEach(() => { cleanup(); vi.resetAllMocks(); });

  it("should debounce the query and select the returned catalogue asset", async () => {
    await user.type(screen.getByRole("textbox", { name: "Search featured items or collections" }), "music");
    expect(searchFeaturedAssets).not.toHaveBeenCalled();
    await user.click(await screen.findByRole("button", { name: /Music wearable\s*Item/ }));
    expect(searchFeaturedAssets).toHaveBeenCalledWith("music", expect.any(AbortSignal));
    expect(onChange).toHaveBeenCalledWith(asset.urn);
  });

  describe("and an earlier search is still pending", () => {
    let resolveFirst: (assets: FeaturedAsset[]) => void;
    let firstSignal: AbortSignal;
    beforeEach(async () => {
      vi.mocked(searchFeaturedAssets).mockImplementationOnce((_query, signal) => {
        firstSignal = signal!;
        return new Promise((resolve) => { resolveFirst = resolve; });
      });
      await user.type(screen.getByRole("textbox", { name: "Search featured items or collections" }), "first");
      await waitFor(() => expect(searchFeaturedAssets).toHaveBeenCalled());
    });
    it("should abort the previous query and ignore its stale results", async () => {
      await user.clear(screen.getByRole("textbox", { name: "Search featured items or collections" }));
      await user.type(screen.getByRole("textbox", { name: "Search featured items or collections" }), "second");
      await screen.findByRole("button", { name: /Music wearable\s*Item/ });
      await act(async () => { resolveFirst([{ ...asset, name: "Stale wearable" }]); });
      expect(firstSignal.aborted).toBe(true);
      expect(screen.queryByRole("button", { name: /Stale wearable\s*Item/ })).toBeNull();
      expect(screen.getByRole("button", { name: /Music wearable\s*Item/ })).toBeTruthy();
    });
    it("should abort the pending lookup when the picker unmounts", () => {
      view.unmount();
      expect(firstSignal.aborted).toBe(true);
    });
  });

  describe("and a catalogue request fails", () => {
    beforeEach(async () => {
      vi.mocked(searchFeaturedAssets).mockRejectedValueOnce(new Error("Catalogue unavailable"));
      await user.type(screen.getByRole("textbox", { name: "Search featured items or collections" }), "music");
      await screen.findByRole("alert");
    });
    it("should retry the same query and show the recovered result", async () => {
      expect(screen.getByRole("alert").textContent).toContain("Catalogue unavailable");
      await user.click(screen.getByRole("button", { name: "Retry asset search" }));
      await screen.findByRole("button", { name: /Music wearable\s*Item/ });
      expect(searchFeaturedAssets).toHaveBeenCalledTimes(2);
      expect(searchFeaturedAssets).toHaveBeenLastCalledWith("music", expect.any(AbortSignal));
      expect(screen.getByRole("button", { name: /Music wearable\s*Item/ })).toBeTruthy();
      expect(screen.queryByRole("alert")).toBeNull();
    });
  });
});

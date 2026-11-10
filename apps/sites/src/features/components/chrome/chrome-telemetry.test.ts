import { describe, expect, it, vi } from "vitest";

import {
  trackFooterLinkClick,
  trackFooterSocialClick,
  trackNavbarClick,
} from "./chrome-telemetry";

const SESSION = { sid: "sid-1", story: "discover" };

describe("chrome telemetry", () => {
  it("sends a navbar Click with source's place, event, action and link, and the delivery fields", () => {
    const track = vi.fn();
    trackNavbarClick(
      { section: "shop", action: "merch", href: "https://store.example/" },
      SESSION,
      track,
      1000,
    );

    expect(track).toHaveBeenCalledWith(
      "Click",
      {
        place: "Landing Navbar",
        event: "click",
        section: "shop",
        action: "merch",
        href: "https://store.example/",
        track_called_at: 1000,
        track_delivered_at: 1000,
        track_deferred: false,
      },
      SESSION,
    );
  });

  it("sends a navbar action Click without a section or href", () => {
    const track = vi.fn();
    trackNavbarClick({ action: "sign_in" }, SESSION, track, 1000);

    expect(track.mock.calls[0]![1]).toEqual({
      place: "Landing Navbar",
      event: "click",
      action: "sign_in",
      track_called_at: 1000,
      track_delivered_at: 1000,
      track_deferred: false,
    });
  });

  it("sends the footer Clicks with exactly source's properties", () => {
    const track = vi.fn();
    trackFooterSocialClick("Discord", SESSION, track);
    trackFooterLinkClick("Download", SESSION, track);

    expect(track.mock.calls).toEqual([
      [
        "Click",
        { place: "Landing Footer Social", event: "click", platform: "Discord" },
        SESSION,
      ],
      [
        "Click",
        { place: "Landing Footer Link", event: "click", link: "Download" },
        SESSION,
      ],
    ]);
  });
});

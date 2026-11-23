import { describe, expect, it } from "vitest";

import { LEGACY_REDIRECT_COOKIE } from "@/core/lib/telemetry/legacy-redirect";

import { loader as discover } from "./discover.$";
import { loader as eventsEvent } from "./events_.event";
import { loader as reportPlayers } from "./report.players";
import { loader as whatsOn } from "./whats-on";
import { loader as whatsOnSplat } from "./whats-on_.$";
import { loader as whatsOnId } from "./whats-on_.$id";

// The old section URLs the legacy site tracked as `Legacy Events Redirected` / `Legacy Places Redirected`
// (decentraland/sites `useLegacyRedirectTracking`): each redirect hands the event's details to the page it lands on.

const ORIGIN = "https://decentraland.example";
const args = (path: string, params: Record<string, string> = {}) =>
  ({ request: new Request(`${ORIGIN}${path}`), params }) as never;

function handedOver(response: Response) {
  const cookie = response.headers.get("Set-Cookie");
  if (!cookie) return null;
  const [pair, ...attributes] = cookie.split("; ");
  const eq = pair!.indexOf("=");
  expect(pair!.slice(0, eq)).toBe(LEGACY_REDIRECT_COOKIE);
  expect(attributes).toEqual(["Path=/", "Max-Age=60", "SameSite=Lax", "Secure"]);
  return JSON.parse(decodeURIComponent(pair!.slice(eq + 1)));
}

describe("a What's On or Discover URL", () => {
  it("/whats-on hands over origin events, its path and query, and where it went", () => {
    const response = whatsOn(args("/whats-on?tab=live&utm_source=x"));
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe("/events?tab=live&utm_source=x");
    expect(handedOver(response)).toEqual({
      origin: "events",
      source: "/whats-on?tab=live&utm_source=x",
      destination: "/events?tab=live&utm_source=x",
      preservedParams: { tab: "live", utm_source: "x" },
    });
  });

  it("/whats-on/:id keeps the old path as the source and the event's page as the destination", () => {
    const response = whatsOnId(args("/whats-on/abc-1", { id: "abc-1" }));
    expect(handedOver(response)).toEqual({
      origin: "events",
      source: "/whats-on/abc-1",
      destination: "/events?id=abc-1",
      preservedParams: {},
    });
  });

  it("a deeper /whats-on path is an events redirect too", () => {
    const response = whatsOnSplat(args("/whats-on/admin/users?tab=x", { "*": "admin/users" }));
    expect(response.headers.get("Location")).toBe("/events/admin/users?tab=x");
    expect(handedOver(response)).toMatchObject({ origin: "events", source: "/whats-on/admin/users?tab=x" });
  });

  it("/events/event, the standalone events site's link, is an events redirect", () => {
    expect(handedOver(eventsEvent(args("/events/event?id=42")))).toEqual({
      origin: "events",
      source: "/events/event?id=42",
      destination: "/events?id=42",
      preservedParams: { id: "42" },
    });
  });

  it("/discover hands over origin places", () => {
    const response = discover(args("/discover/world/foo.dcl.eth?tab=about"));
    expect(response.headers.get("Location")).toBe("/places/world/foo.dcl.eth?tab=about");
    expect(handedOver(response)).toEqual({
      origin: "places",
      source: "/discover/world/foo.dcl.eth?tab=about",
      destination: "/places/world/foo.dcl.eth?tab=about",
      preservedParams: { tab: "about" },
    });
  });

  it("drops a credential from the source, the destination and the params", () => {
    const details = handedOver(whatsOn(args("/whats-on?token=secret&tab=live")));
    expect(details).toEqual({
      origin: "events",
      source: "/whats-on?tab=live",
      destination: "/events?tab=live",
      preservedParams: { tab: "live" },
    });
  });

  it("still redirects, handing nothing over, when the details would not fit in a cookie", () => {
    const long = "a".repeat(4000);
    const response = whatsOn(args(`/whats-on?q=${long}`));
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe(`/events?q=${long}`);
    expect(response.headers.get("Set-Cookie")).toBeNull();
  });
});

describe("/report/players", () => {
  it("redirects to the form without a legacy redirect event, as decentraland.org's plain Navigate did", () => {
    const response = reportPlayers(args("/report/players?reported_address=0x1"));
    expect(response.headers.get("Location")).toBe("/report?reported_address=0x1");
    expect(response.headers.get("Set-Cookie")).toBeNull();
  });
});

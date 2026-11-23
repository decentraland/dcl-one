// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// decentraland.org's invite `Click` (decentraland/sites `InviteHero.tsx:94-101` over `useTrackClick`), through the
// real transport and gates. Each test loads a fresh document, whose readiness is its first page view.

const BROWSER_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const BOT_UA = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";
const INVITE = "/invite/0x1111111111111111111111111111111111111111";

const sendBeacon = vi.fn((_url: string, _body: Blob) => true);

function browser(userAgent = BROWSER_UA) {
  vi.stubGlobal("navigator", { userAgent, language: "en-US", maxTouchPoints: 0, sendBeacon });
}

async function loadDocument(path = INVITE) {
  window.history.replaceState(null, "", path);
  vi.spyOn(performance, "getEntriesByType").mockReturnValue([]);
  vi.resetModules();
  const { page } = await import("@/core/lib/telemetry/track");
  const tracked = await import("@/core/lib/telemetry/tracked-user");
  const { trackInviteClick } = await import("./invite-click");
  return { page, trackInviteClick, dropTrackedUser: tracked.dropTrackedUser };
}

async function clicks(): Promise<Array<Record<string, unknown>>> {
  const bodies = await Promise.all(sendBeacon.mock.calls.map(async ([, blob]) => JSON.parse(await blob.text())));
  return bodies.filter((b) => b.type === "track" && b.event === "Click");
}

beforeEach(() => {
  (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "key-test" };
  browser();
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  sendBeacon.mockClear();
  delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
  window.history.replaceState(null, "", "/");
});

describe("trackInviteClick", () => {
  it("sends the first hero's Click with only its place, the URL's utm_* and the audit fields", async () => {
    const { page, trackInviteClick } = await loadDocument(`${INVITE}?utm_source=Friend%20Link&utm_campaign=Invite`);
    page(INVITE, {}, { sid: "sid-1" });
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackInviteClick("first", { sid: "sid-1", userId: "0xAbC" });
    const [click] = await clicks();
    expect(click).toMatchObject({ anonymousId: "sid-1", userId: "0xabc" });
    expect(click!.properties).toEqual({
      utm_source: "friend_link",
      utm_campaign: "invite",
      place: "Referrer Invite First Hero",
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_000,
      track_deferred: false,
    });
  });

  it("names the second hero by its own place", async () => {
    const { page, trackInviteClick } = await loadDocument();
    page(INVITE, {}, { sid: "sid-1" });
    trackInviteClick("second", { sid: "sid-1" });
    const [click] = await clicks();
    expect(click!.properties).toMatchObject({ place: "Referrer Invite Second Hero" });
    expect(click).not.toHaveProperty("userId");
  });

  it("holds a click made before the page view and sends it deferred once the page view leaves", async () => {
    const { page, trackInviteClick } = await loadDocument();
    const now = vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackInviteClick("first", { sid: "sid-1" });
    expect(await clicks()).toEqual([]);
    now.mockReturnValue(1_700_000_000_300);
    page(INVITE, {}, { sid: "sid-1" });
    const [click] = await clicks();
    expect(click!.properties).toMatchObject({
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_300,
      track_deferred: true,
    });
  });

  it("names no account once a sign-out has started", async () => {
    const { page, trackInviteClick, dropTrackedUser } = await loadDocument();
    page(INVITE, {}, { sid: "sid-1" });
    dropTrackedUser();
    trackInviteClick("first", { sid: "sid-1", userId: "0xabc" });
    expect((await clicks())[0]).not.toHaveProperty("userId");
  });

  it("never sends a bot's click", async () => {
    browser(BOT_UA);
    const { page, trackInviteClick } = await loadDocument();
    page(INVITE, {}, { sid: "sid-1" });
    trackInviteClick("first", { sid: "sid-1" });
    expect(sendBeacon).not.toHaveBeenCalled();
  });

  it("never sends a click in a session that started on an exempt page", async () => {
    const { page, trackInviteClick } = await loadDocument("/referral-terms");
    page("/referral-terms", {}, { sid: "sid-1" });
    window.history.replaceState(null, "", INVITE);
    page(INVITE, {}, { sid: "sid-1" });
    trackInviteClick("first", { sid: "sid-1" });
    expect(sendBeacon).not.toHaveBeenCalled();
  });

  it("sends nothing without a session id", async () => {
    const { page, trackInviteClick } = await loadDocument();
    page(INVITE, {}, { sid: "sid-1" });
    sendBeacon.mockClear();
    trackInviteClick("first", { sid: "" });
    expect(sendBeacon).not.toHaveBeenCalled();
  });
});

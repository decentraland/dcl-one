import { describe, expect, it } from "vitest";

import { isCastPublisher, pickWatcherStage, streamerStage, type StageCandidate } from "./stage";

const t = (identity: string, source: StageCandidate["source"], extra: Partial<StageCandidate> = {}) => ({
  identity,
  source,
  muted: false,
  local: false,
  ...extra,
});

describe("pickWatcherStage", () => {
  it("prefers a Cast screen share, then any screen share, then a Cast camera, then any camera", () => {
    const wallet = "0xabc";
    const streamer = "stream:place-1:uuid";
    const bot = "presentation-bot:room:uuid";
    expect(pickWatcherStage([t(wallet, "camera"), t(streamer, "camera")])?.identity).toBe(streamer);
    expect(pickWatcherStage([t(streamer, "camera"), t(wallet, "screen_share")])?.identity).toBe(wallet);
    expect(
      pickWatcherStage([t(wallet, "screen_share"), t(bot, "screen_share"), t(streamer, "camera")])?.identity,
    ).toBe(bot);
    expect(pickWatcherStage([t(wallet, "camera")])?.identity).toBe(wallet);
  });

  it("never shows a muted track, audio, or nothing at all", () => {
    expect(pickWatcherStage([])).toBeNull();
    expect(pickWatcherStage([t("stream:a:b", "microphone"), t("stream:a:b", "screen_share_audio")])).toBeNull();
    expect(
      pickWatcherStage([t("stream:a:b", "screen_share", { muted: true }), t("0x1", "camera")])?.identity,
    ).toBe("0x1");
  });

  it("keeps the first of equal rank", () => {
    const a = t("stream:p:1", "screen_share");
    const b = t("stream:p:2", "screen_share");
    expect(pickWatcherStage([a, b])).toBe(a);
  });
});

describe("streamerStage", () => {
  it("shows only the streamer's own unmuted screen then camera", () => {
    const cam = t("stream:p:1", "camera", { local: true });
    const screen = t("stream:p:1", "screen_share", { local: true });
    const remote = t("0x1", "screen_share");
    const mutedMic = t("stream:p:1", "microphone", { local: true });
    expect(streamerStage([cam, remote, screen, mutedMic])).toEqual([screen, cam]);
    expect(streamerStage([{ ...cam, muted: true }])).toEqual([]);
  });
});

describe("isCastPublisher", () => {
  it("names the gatekeeper's streamer and presentation-bot identities only", () => {
    expect(isCastPublisher("stream:place:uuid")).toBe(true);
    expect(isCastPublisher("presentation-bot:room:uuid")).toBe(true);
    expect(isCastPublisher("watch:room:uuid")).toBe(false);
    expect(isCastPublisher("0xabc")).toBe(false);
  });
});

import { describe, expect, it } from "vitest";

import { isCastPublisher, isPerson, stalledCast, streamerStage, watcherStage, type StageCandidate } from "./stage";

const t = (identity: string, source: StageCandidate["source"], extra: Partial<StageCandidate> = {}) => ({
  identity,
  source,
  muted: false,
  local: false,
  stalled: false,
  picture: true,
  key: `${identity}/${source}`,
  ...extra,
});
const pickWatcherStage = (tracks: Parameters<typeof watcherStage>[0]) => watcherStage(tracks, null).shown;

describe("watcherStage", () => {
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

  it("keeps a screen share on the stage while it is published, stalled or not", () => {
    const camera = t("stream:place-1:uuid", "camera");
    const share = t("stream:place-1:uuid", "screen_share", { stalled: true });
    expect(watcherStage([camera, share], share.key)).toEqual({ shown: share, next: null });
    expect(watcherStage([camera, { ...share, muted: true }], share.key)).toEqual({ shown: camera, next: null });
    expect(watcherStage([camera], share.key)).toEqual({ shown: camera, next: null });
  });

  it("gives the stage to a track of a better rank only with its first picture, and never to one of the same or a lower rank", () => {
    const camera = t("stream:place-1:uuid", "camera");
    const share = t("stream:place-1:uuid", "screen_share", { picture: false });
    expect(watcherStage([camera, share], camera.key)).toEqual({ shown: camera, next: share });
    const drawn = { ...share, picture: true };
    expect(watcherStage([camera, drawn], camera.key)).toEqual({ shown: drawn, next: null });
    expect(watcherStage([camera, drawn], drawn.key)).toEqual({ shown: drawn, next: null });
    const other = t("stream:place-2:uuid", "camera");
    expect(watcherStage([camera, other], other.key)).toEqual({ shown: other, next: null });
  });

  it("shows the best track, with or without a picture, when the stage has no owner", () => {
    const share = t("stream:place-1:uuid", "screen_share", { picture: false });
    expect(watcherStage([t("stream:place-1:uuid", "camera"), share], null)).toEqual({ shown: share, next: null });
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

describe("isPerson", () => {
  it("counts a watcher, a wallet in the scene and a streamer that publishes", () => {
    expect(isPerson("watch:room:uuid", false)).toBe(true);
    expect(isPerson("0xabc", false)).toBe(true);
    expect(isPerson("stream:place:access:uuid", true)).toBe(true);
  });

  it("never counts the presentation bot, the RTMP ingress, the scene's server or a stream key's connection that publishes nothing", () => {
    expect(isPerson("authoritative-server", true)).toBe(false);
    expect(isPerson("presentation-bot:room:access:uuid", true)).toBe(false);
    expect(isPerson("0b9d2f6e-1c57-4c1e-9a53-0d7c1c2a4e11-streamer", true)).toBe(false);
    expect(isPerson("stream:place:access:uuid", false)).toBe(false);
  });
});

describe("stalledCast", () => {
  const streamer = "stream:place-1:uuid";
  const frozen = { stalled: true };

  it("is true when the picture on the stage does not advance, whatever the sound and the other pictures do", () => {
    const camera = t(streamer, "camera");
    expect(stalledCast([camera, t(streamer, "microphone")], { ...camera, ...frozen })).toBe(true);
    expect(stalledCast([t(streamer, "screen_share", frozen), t(streamer, "microphone", frozen)], camera)).toBe(false);
  });

  it("with no picture, is true when no sound advances", () => {
    expect(stalledCast([t(streamer, "microphone", frozen)], null)).toBe(true);
    expect(stalledCast([t(streamer, "microphone", frozen), t("0x1", "microphone")], null)).toBe(false);
  });

  it("is false with nothing on air, and never reads the page's own tracks", () => {
    expect(stalledCast([], null)).toBe(false);
    expect(stalledCast([t(streamer, "camera", { local: true, stalled: true })], null)).toBe(false);
  });
});

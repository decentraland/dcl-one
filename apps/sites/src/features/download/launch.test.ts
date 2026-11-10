// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import { desktopAppUrl, launchDesktopApp, launchFlags } from "./launch";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("desktopAppUrl", () => {
  it("forwards position, realm and the community id under ui2's `community` key", () => {
    expect(
      desktopAppUrl({
        position: "10,20",
        realm: "main",
        communityId: "abc-1",
        dclenv: "zone",
      }),
    ).toBe(
      "decentraland://?position=10%2C20&realm=main&community=abc-1&dclenv=zone",
    );
  });

  it("maps the scene console and multi-instance flags to their deep link keys", () => {
    expect(desktopAppUrl({ sceneConsole: "true", multiInstance: "true" })).toBe(
      "decentraland://?scene-console=true&multi-instance=true",
    );
  });
});

describe("launchFlags", () => {
  it("accepts only known environments and the literal true flags", () => {
    expect(
      launchFlags(
        new URLSearchParams(
          "dclenv=today&scene-console=true&multi-instance=true",
        ),
      ),
    ).toEqual({
      dclenv: "today",
      sceneConsole: "true",
      multiInstance: "true",
    });
    expect(
      launchFlags(
        new URLSearchParams("dclenv=evil.example&multi-instance=false"),
      ),
    ).toEqual({
      dclenv: undefined,
      sceneConsole: undefined,
      multiInstance: undefined,
    });
  });

  it("maps env to dclenv when dclenv is absent", () => {
    expect(launchFlags(new URLSearchParams("env=stg")).dclenv).toBe("today");
    expect(
      launchFlags(new URLSearchParams("env=other")).dclenv,
    ).toBeUndefined();
  });
});

describe("launchDesktopApp", () => {
  function stubAssign(onAssign: () => void = () => undefined) {
    const assign = vi.fn(onAssign);
    vi.spyOn(window, "location", "get").mockReturnValue({
      ...window.location,
      assign,
    } as Location);
    return assign;
  }

  it("resolves true when the page loses focus within the timeout", async () => {
    vi.useFakeTimers();
    const assign = stubAssign(() => window.dispatchEvent(new Event("blur")));

    const launched = launchDesktopApp({ position: "1,1" });
    await vi.advanceTimersByTimeAsync(750);

    await expect(launched).resolves.toBe(true);
    expect(assign).toHaveBeenCalledWith("decentraland://?position=1%2C1");
  });

  it("resolves false when nothing takes over", async () => {
    vi.useFakeTimers();
    stubAssign();

    const launched = launchDesktopApp();
    await vi.advanceTimersByTimeAsync(750);

    await expect(launched).resolves.toBe(false);
  });

  it("does not try on mobile", async () => {
    const assign = stubAssign();
    await expect(launchDesktopApp({ mobile: true })).resolves.toBe(false);
    expect(assign).not.toHaveBeenCalled();
  });

  it("resolves false when the scheme throws", async () => {
    stubAssign(() => {
      throw new Error("unknown scheme");
    });
    await expect(launchDesktopApp()).resolves.toBe(false);
  });
});

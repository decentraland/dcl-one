import { beforeEach, describe, expect, it } from "vitest";
import { formatHomeStarted, homeLiveActivity } from "./locale";

describe("when formatting Home live activity in English", () => {
  let activity: ReturnType<typeof homeLiveActivity>;

  beforeEach(() => {
    activity = homeLiveActivity("en");
  });

  it("should use the just-started label below one minute", () => {
    expect(formatHomeStarted("en", 0)).toBe("Just started");
  });

  it("should use whole elapsed hours for multi-hour events", () => {
    expect(formatHomeStarted("en", 119)).toBe("Started 1 hr. ago");
  });

  it("should use elapsed days for multi-day events", () => {
    expect(formatHomeStarted("en", 2_881)).toBe("Started 2 days ago");
  });

  it("should identify the event named by the copy-link action", () => {
    expect(activity.copyLink("Open Mic")).toBe("Copy event link: Open Mic");
  });
});


describe("when the locale puts the elapsed time first", () => {
  it("should use Chinese word order", () => {
    expect(formatHomeStarted("zh", 5)).toMatch(/前开始$/);
  });

  it("should use Japanese word order", () => {
    expect(formatHomeStarted("ja", 5)).toMatch(/前に開始$/);
  });

  it("should use Korean word order", () => {
    expect(formatHomeStarted("ko", 5)).toMatch(/전 시작$/);
  });
});

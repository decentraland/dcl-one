import { beforeEach, describe, expect, it } from "vitest";
import { homeLiveActivity } from "./locale";

describe("when formatting Home live activity in English", () => {
  let activity: ReturnType<typeof homeLiveActivity>;

  beforeEach(() => {
    activity = homeLiveActivity("en");
  });

  it("should use the just-started label below one minute", () => {
    expect(activity.started(0)).toBe("Just started");
  });

  it("should use whole elapsed hours for multi-hour events", () => {
    expect(activity.started(119)).toBe("Started 1 hr. ago");
  });

  it("should use elapsed days for multi-day events", () => {
    expect(activity.started(2_881)).toBe("Started 2 days ago");
  });

  it("should identify the event named by the copy-link action", () => {
    expect(activity.copyLink("Open Mic")).toBe("Copy event link: Open Mic");
  });
});

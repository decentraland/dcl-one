// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { emptyDraft, type HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import SubmitHangout from "./SubmitHangout";

vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });

const DISCLAIMER = "Daily events limited to 2 hours. Longer ones will be rejected in review.";

function renderSchedule(draft: HangoutDraft, canScheduleLongDailyEvents = false) {
  render(<MemoryRouter initialEntries={["/edit?step=schedule"]}>
    <SubmitHangout
      trackCtx={{ sid: "test", story: "landings/submit-hangout", variant: "wizard" }}
      categories={[]}
      signedIn
      draft={draft}
      canScheduleLongDailyEvents={canScheduleLongDailyEvents}
      track={vi.fn()}
    />
  </MemoryRouter>);
}

describe("when a creator schedules a hangout that repeats every day", () => {
  beforeEach(() => {
    renderSchedule({
      ...emptyDraft(),
      startDate: "2030-01-01",
      startTime: "18:00",
      durationHours: 3,
      recurrent: true,
      recurrence: "every_day",
      recurrentUntil: "2030-02-01",
    });
  });
  afterEach(cleanup);

  it("should show the daily-duration disclaimer", () => {
    expect(screen.getByText(DISCLAIMER)).toBeTruthy();
  });

  it("should keep the hangout submittable, since the disclaimer blocks nothing", () => {
    expect((screen.getByRole("button", { name: "Continue" }) as HTMLButtonElement).disabled).toBe(false);
  });
});

describe("when a creator switches a daily hangout to repeat every week", () => {
  beforeEach(async () => {
    renderSchedule({ ...emptyDraft(), recurrent: true, recurrence: "every_day" });
    await userEvent.setup().selectOptions(screen.getByRole("combobox", { name: "Repeat" }), "every_week");
  });
  afterEach(cleanup);

  it("should hide the disclaimer", () => {
    expect(screen.queryByText(DISCLAIMER)).toBeNull();
  });
});

describe("when a hangout does not repeat", () => {
  beforeEach(() => {
    renderSchedule({ ...emptyDraft(), recurrent: false, recurrence: "every_day" });
  });
  afterEach(cleanup);

  it("should not show the disclaimer", () => {
    expect(screen.queryByText(DISCLAIMER)).toBeNull();
  });
});

describe("when a team member schedules a hangout that repeats every day", () => {
  beforeEach(() => {
    renderSchedule({ ...emptyDraft(), durationHours: 24, recurrent: true, recurrence: "every_day" }, true);
  });
  afterEach(cleanup);

  it("should not show the disclaimer", () => {
    expect(screen.queryByText(DISCLAIMER)).toBeNull();
  });
});

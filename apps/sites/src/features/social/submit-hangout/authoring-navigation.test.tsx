// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TrackFn } from "./machine";
import SubmitHangout from "./SubmitHangout";
import ScheduleWizard from "../../explorer/event-schedule/ScheduleWizard";

vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });

describe.each(["hangout", "schedule"] as const)("when signed in to the %s wizard", (kind) => {
  let router: ReturnType<typeof createMemoryRouter>;
  let track: ReturnType<typeof vi.fn<TrackFn>>;
  let user: ReturnType<typeof userEvent.setup>;
  let upload: ReturnType<typeof vi.fn<(file: File, vertical: boolean, signal?: AbortSignal) => Promise<string>>>;

  beforeEach(() => {
    track = vi.fn();
    user = userEvent.setup();
    upload = vi.fn(() => new Promise<string>(() => {}));
    const trackCtx = { sid: "test", story: kind === "hangout" ? "landings/submit-hangout" : "landings/event-schedule", variant: "wizard" };
    router = createMemoryRouter([
      { path: "/edit", element: kind === "hangout"
        ? <SubmitHangout trackCtx={trackCtx} categories={[]} signedIn track={track} upload={upload} />
        : <ScheduleWizard trackCtx={trackCtx} schedules={[]} source="empty" signedIn track={track} /> },
      { path: "/whats-on", element: <h1>What's On</h1> },
    ], { initialEntries: ["/edit"] });
    render(<RouterProvider router={router} />);
  });
  afterEach(() => { cleanup(); router.dispose(); vi.restoreAllMocks(); });

  it("should leave the first step without revisiting the sign-in gate or starting again", async () => {
    await user.click(await screen.findByRole("link", { name: "Back" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/whats-on"));
    expect(track.mock.calls.filter(([event]) => event === (kind === "hangout" ? "lp_hangout_started" : "lp_schedule_started"))).toHaveLength(1);
    expect(track.mock.calls.filter(([event]) => event === (kind === "hangout" ? "lp_hangout_signin_gate_viewed" : "lp_schedule_gate_viewed"))).toHaveLength(1);
  });

  if (kind === "hangout") {
    describe("and a poster is still uploading", () => {
      beforeEach(async () => {
        await user.upload(await screen.findByLabelText("Upload cover image"), new File(["image"], "poster.png", { type: "image/png" }));
      });
      it("should abort the upload when leaving the editor", async () => {
        await user.click(screen.getByRole("link", { name: "Back" }));
        await waitFor(() => expect(router.state.location.pathname).toBe("/whats-on"));
        expect((upload.mock.calls[0]?.[2] as AbortSignal).aborted).toBe(true);
      });
    });
  }
});

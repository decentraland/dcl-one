// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TrackFn } from "./machine";
import SubmitHangout from "./SubmitHangout";

vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });

describe("when signed in to the hangout wizard", () => {
  let router: ReturnType<typeof createMemoryRouter>;
  let track: ReturnType<typeof vi.fn<TrackFn>>;
  let user: ReturnType<typeof userEvent.setup>;
  let upload: ReturnType<typeof vi.fn<(file: File, vertical: boolean, signal?: AbortSignal) => Promise<string>>>;

  beforeEach(() => {
    track = vi.fn();
    user = userEvent.setup();
    upload = vi.fn(() => new Promise<string>(() => {}));
    const trackCtx = { sid: "test", story: "landings/submit-hangout", variant: "wizard" };
    router = createMemoryRouter([
      { path: "/edit", element: <SubmitHangout trackCtx={trackCtx} categories={[]} signedIn track={track} upload={upload} /> },
      { path: "/events", element: <h1>Events</h1> },
    ], { initialEntries: ["/edit"] });
    render(<RouterProvider router={router} />);
  });
  afterEach(() => { cleanup(); router.dispose(); vi.restoreAllMocks(); });

  it("should leave the first step without revisiting the sign-in gate or starting again", async () => {
    await user.click(await screen.findByRole("link", { name: "Back" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/events"));
    expect(track.mock.calls.filter(([event]) => event === "lp_hangout_started")).toHaveLength(1);
    expect(track.mock.calls.filter(([event]) => event === "lp_hangout_signin_gate_viewed")).toHaveLength(1);
  });

  describe("and a poster is still uploading", () => {
    beforeEach(async () => {
      await user.upload(await screen.findByLabelText("Upload cover image"), new File(["image"], "poster.png", { type: "image/png" }));
    });
    it("should abort the upload when leaving the editor", async () => {
      await user.click(screen.getByRole("link", { name: "Back" }));
      await waitFor(() => expect(router.state.location.pathname).toBe("/events"));
      expect((upload.mock.calls[0]?.[2] as AbortSignal).aborted).toBe(true);
    });
  });
});

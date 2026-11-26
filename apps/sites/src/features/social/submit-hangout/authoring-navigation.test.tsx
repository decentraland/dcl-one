// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
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
    it("should show a spinner named for the upload while it runs", () => {
      expect(screen.getByRole("progressbar", { name: "Uploading image…" }).closest("[role=status]")).toBeTruthy();
      expect(screen.queryByRole("alert")).toBeNull();
    });

    it("should abort the upload when leaving the editor", async () => {
      await user.click(screen.getByRole("link", { name: "Back" }));
      await waitFor(() => expect(router.state.location.pathname).toBe("/events"));
      expect((upload.mock.calls[0]?.[2] as AbortSignal).aborted).toBe(true);
    });
  });
});

describe("when the hangout wizard reaches its details and location steps", () => {
  let user: ReturnType<typeof userEvent.setup>;
  let options: HangoutOptions;
  const trackCtx = { sid: "test", story: "landings/submit-hangout", variant: "wizard" };
  const categories = [{ name: "music", label: "Music" }];

  function open(step: string) {
    const router = createMemoryRouter([
      { path: "/edit", element: <SubmitHangout trackCtx={trackCtx} categories={categories} signedIn options={options} track={vi.fn()} /> },
    ], { initialEntries: [`/edit?step=${step}`] });
    render(<RouterProvider router={router} />);
    return router;
  }

  beforeEach(() => {
    user = userEvent.setup();
    options = { profile: null, profileError: null, communities: [], communitiesError: null, worlds: ["music.dcl.eth"], worldsError: null };
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("should name the category select by its label and keep the chosen category", async () => {
    open("details");
    const category = screen.getByRole<HTMLSelectElement>("combobox", { name: "Category" });
    expect(category.value).toBe("");
    await user.selectOptions(category, "music");
    expect(category.value).toBe("music");
  });

  it("should take LAND coordinates as numbers", async () => {
    open("location");
    const x = screen.getByRole<HTMLInputElement>("spinbutton", { name: "X" });
    await user.clear(x);
    await user.type(x, "12");
    expect(x.value).toBe("12");
    expect(screen.getByRole("spinbutton", { name: "Y" })).toHaveProperty("value", "0");
  });

  it("should suggest the account's worlds for a world hangout", async () => {
    open("location");
    await user.selectOptions(screen.getByRole("combobox", { name: "Location Type" }), "world");
    const world = screen.getByLabelText("World");
    expect(world.getAttribute("list")).toBe("hangout-worlds");
    expect(document.querySelector('#hangout-worlds option[value="music.dcl.eth"]')).toBeTruthy();
    expect(world.getAttribute("aria-describedby")).toBeNull();
  });

  it("should describe the world field with the failure when suggestions could not load", async () => {
    options = { ...options, worlds: [], worldsError: "Worlds unavailable" };
    open("location");
    await user.selectOptions(screen.getByRole("combobox", { name: "Location Type" }), "world");
    const description = document.getElementById(screen.getByLabelText("World").getAttribute("aria-describedby") ?? "");
    expect(description?.textContent).toBe("World suggestions could not load. You can enter the world name.");
  });
});

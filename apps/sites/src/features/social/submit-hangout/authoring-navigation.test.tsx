// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { emptyDraft, type SubmitFn } from "@/data/lib/catalyst/landings/submit-hangout";
import type { HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import type { TrackFn } from "./machine";
import SubmitHangout from "./SubmitHangout";

vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });

// The form passes variant="tertiary"; Button draws it with its outline look and reports data-variant="outline".

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

  it("should draw Back as an outlined link to the events page and Continue as the primary button", async () => {
    const back = await screen.findByRole("link", { name: "Back" });
    expect(back.getAttribute("href")).toBe("/events");
    expect(back.dataset).toMatchObject({ variant: "outline", size: "md" });
    expect(screen.getByRole("button", { name: "Continue" }).dataset).toMatchObject({ variant: "primary", size: "md" });
  });

  it("should leave a modified click on Back to the browser", async () => {
    const back = await screen.findByRole("link", { name: "Back" });
    back.addEventListener("click", (event) => event.preventDefault());
    await user.keyboard("{Meta>}");
    await user.click(back);
    await user.keyboard("{/Meta}");
    expect(router.state.location.pathname).toBe("/edit");
  });

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

    it("should not move on while the poster uploads", async () => {
      const next = screen.getByRole<HTMLButtonElement>("button", { name: "Continue" });
      expect(next.disabled).toBe(true);
      await user.click(next);
      expect(router.state.location.search).toBe("?step=cover");
    });

    it("should abort the upload when leaving the editor", async () => {
      await user.click(screen.getByRole("link", { name: "Back" }));
      await waitFor(() => expect(router.state.location.pathname).toBe("/events"));
      expect((upload.mock.calls[0]?.[2] as AbortSignal).aborted).toBe(true);
    });
  });
});

describe("when the hangout wizard reaches its review step", () => {
  let submit: ReturnType<typeof vi.fn<SubmitFn>>;
  let user: ReturnType<typeof userEvent.setup>;
  let router: ReturnType<typeof createMemoryRouter>;

  beforeEach(() => {
    submit = vi.fn<SubmitFn>(() => new Promise(() => {}));
    user = userEvent.setup();
    const draft = { ...emptyDraft(), name: "Open studio", startDate: "2099-07-01", startTime: "18:30", coordX: 10, coordY: 20 };
    const trackCtx = { sid: "test", story: "landings/submit-hangout", variant: "wizard" };
    router = createMemoryRouter([
      { path: "/edit", element: <SubmitHangout trackCtx={trackCtx} categories={[]} signedIn draft={draft} submit={submit} track={vi.fn()} /> },
    ], { initialEntries: ["/edit?step=review"] });
    render(<RouterProvider router={router} />);
  });
  afterEach(() => { cleanup(); router.dispose(); vi.restoreAllMocks(); });

  it("should draw Back and Preview as outlined buttons and Submit Hangout as the primary one", () => {
    expect(screen.getByRole("button", { name: "Back" }).dataset).toMatchObject({ variant: "outline", size: "md" });
    expect(screen.getByRole("button", { name: "Preview" }).dataset).toMatchObject({ variant: "outline", size: "md" });
    expect(screen.getByRole("button", { name: "Submit Hangout" }).dataset).toMatchObject({ variant: "primary", size: "md" });
  });

  it("should return to the schedule step when Back is pressed, without submitting", async () => {
    await user.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(router.state.location.search).toBe("?step=schedule"));
    expect(screen.getByLabelText("Date")).toHaveProperty("value", "2099-07-01");
    expect(submit).not.toHaveBeenCalled();
  });

  it("should submit the draft when Submit Hangout is pressed", async () => {
    await user.click(screen.getByRole("button", { name: "Submit Hangout" }));
    expect(submit).toHaveBeenCalledWith(expect.objectContaining({ draft: expect.objectContaining({ name: "Open studio" }) }));
  });

  it("should submit from the preview step with Confirm & Submit", async () => {
    await user.click(screen.getByRole("button", { name: "Preview" }));
    await waitFor(() => expect(router.state.location.search).toBe("?step=preview"));
    expect(submit).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Confirm & Submit" }));
    expect(submit).toHaveBeenCalledTimes(1);
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

  it("should keep the LAND coordinate entered in X", () => {
    open("location");
    const x = screen.getByRole<HTMLInputElement>("spinbutton", { name: "X" });
    fireEvent.change(x, { target: { value: "12" } });
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

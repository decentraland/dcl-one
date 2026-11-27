// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, MemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { emptyDraft, type SubmitFn } from "@/data/lib/catalyst/landings/submit-hangout";
import SubmitHangout from "./SubmitHangout";
import { HANGOUT_EVENTS, type TrackFn } from "./machine";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

// The form passes variant="tertiary"; Button draws it with its outline look and reports data-variant="outline".

describe("when a signed-in creator opens the single-page Events editor", () => {
  let user: ReturnType<typeof userEvent.setup>;
  let submit: ReturnType<typeof vi.fn<SubmitFn>>;
  let upload: ReturnType<typeof vi.fn<(file: File, vertical: boolean, signal?: AbortSignal) => Promise<string>>>;
  let image: File;

  beforeEach(() => {
    user = userEvent.setup();
    submit = vi.fn<SubmitFn>().mockResolvedValue({ id: "new-event", approved: false });
    upload = vi.fn<(file: File, vertical: boolean, signal?: AbortSignal) => Promise<string>>().mockResolvedValue("https://example.test/poster.png");
    image = new File(["cover"], "cover.png", { type: "image/png" });
    render(
      <MemoryRouter initialEntries={["/events/new-event"]}>
        <SubmitHangout
          editor
          signedIn
          trackCtx={{ sid: "test", story: "landings/submit-hangout", variant: "wizard" }}
          categories={[]}
          submit={submit}
          upload={upload}
          track={vi.fn()}
        />
      </MemoryRouter>,
    );
  });

  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
  });

  it("should label the event details section once", () => {
    expect(screen.getAllByText("Event details")).toHaveLength(1);
    expect(screen.getByRole("group", { name: "Event details" })).toBeTruthy();
  });

  it("should keep each field's limits, type and rows", () => {
    expect(screen.getByRole("textbox", { name: "Event Name" })).toHaveProperty("maxLength", 150);
    expect(screen.getByRole("textbox", { name: "Event Description" })).toHaveProperty("maxLength", 5000);
    expect(screen.getByRole("textbox", { name: "Details for moderators" })).toHaveProperty("maxLength", 5000);
    expect(screen.getByRole("textbox", { name: "Event Description" })).toHaveProperty("rows", 3);
    expect(screen.getByRole("textbox", { name: "Details for moderators" })).toHaveProperty("rows", 3);
    expect(screen.getByRole("textbox", { name: "Contact email" })).toHaveProperty("type", "email");
    const duration = screen.getByRole("spinbutton", { name: "Duration (hours)" });
    expect([duration.getAttribute("min"), duration.getAttribute("max"), duration.getAttribute("step")]).toEqual(["0.25", "24", "0.25"]);
    expect(screen.getByLabelText("Date")).toHaveProperty("type", "date");
    expect(screen.getByLabelText("Start (UTC)")).toHaveProperty("type", "time");
  });

  it("should show the repeat options only while Repeat Event is on", async () => {
    const repeat = screen.getByRole<HTMLInputElement>("switch", { name: "Repeat Event" });
    expect(repeat.checked).toBe(false);
    expect(screen.queryByRole("combobox", { name: "Repeat" })).toBeNull();
    expect(screen.queryByLabelText("Ends")).toBeNull();
    await user.click(repeat);
    expect(repeat.checked).toBe(true);
    await user.selectOptions(screen.getByRole("combobox", { name: "Repeat" }), "every_month");
    expect(screen.getByRole<HTMLSelectElement>("combobox", { name: "Repeat" }).value).toBe("every_month");
    expect(screen.getByLabelText("Ends")).toHaveProperty("type", "date");
    await user.click(repeat);
    expect(repeat.checked).toBe(false);
    expect(screen.queryByRole("combobox", { name: "Repeat" })).toBeNull();
    expect(screen.queryByLabelText("Ends")).toBeNull();
  });

  describe("and the required details are incomplete", () => {
    it("should block the preview", () => {
      expect(screen.getByRole<HTMLButtonElement>("button", { name: "Preview" }).disabled).toBe(true);
    });

    it("should block the submission", () => {
      expect(screen.getByRole<HTMLButtonElement>("button", { name: "Submit Event" }).disabled).toBe(true);
    });
  });

  it("should use the same upload path for dropping a cover", async () => {
    fireEvent.drop(screen.getByText("Upload cover image").closest("label")!, {
      dataTransfer: { files: [image] },
    });
    const cover = await screen.findByRole("img", { name: "Hangout cover" });
    expect(cover.getAttribute("src")).toBe("https://example.test/poster.png");
    expect(upload).toHaveBeenCalledWith(image, false, expect.any(AbortSignal));
  });

  it("should keep the vertical picker visible after removing a cover", async () => {
    const summary = screen.getByText("Add Vertical Cover").closest("summary")!;
    await user.click(summary);
    const picker = screen.getByLabelText("Upload vertical cover");
    await user.upload(picker, image);
    await user.click(await screen.findByRole("button", { name: "Remove vertical cover" }));
    expect(summary.parentElement).toHaveProperty("open", true);
    await user.upload(screen.getByLabelText("Upload vertical cover"), image);
    expect(await screen.findByRole("img", { name: "Vertical hangout cover" })).toBeTruthy();
  });

  it("should draw Remove cover image as an outlined button that removes the cover", async () => {
    await user.upload(screen.getByLabelText("Upload cover image"), image);
    const remove = await screen.findByRole("button", { name: "Remove cover image" });
    expect(remove.dataset).toMatchObject({ variant: "outline", size: "md" });
    await user.click(remove);
    expect(screen.queryByRole("img", { name: "Hangout cover" })).toBeNull();
  });

  describe("and a cover is replaced while the new one uploads", () => {
    beforeEach(async () => {
      await user.click(screen.getByText("Add Vertical Cover").closest("summary")!);
      await user.upload(screen.getByLabelText("Upload vertical cover"), image);
      await screen.findByRole("img", { name: "Vertical hangout cover" });
      upload.mockImplementationOnce(() => new Promise<string>(() => {}));
      await user.upload(screen.getByLabelText("Upload cover image"), image);
    });

    it("should keep Remove vertical cover disabled until the upload ends", async () => {
      const remove = screen.getByRole<HTMLButtonElement>("button", { name: "Remove vertical cover" });
      expect(remove.disabled).toBe(true);
      await user.click(remove);
      expect(screen.getByRole("img", { name: "Vertical hangout cover" })).toBeTruthy();
    });
  });

  describe("and the first cover upload fails", () => {
    beforeEach(() => {
      upload.mockRejectedValueOnce(new Error("Cover upload failed"));
    });

    it("should let the creator retry the same file", async () => {
      const picker = screen.getByLabelText("Upload cover image");
      await user.upload(picker, image);
      expect((await screen.findByRole("alert")).textContent).toBe("Cover upload failed");
      expect(screen.getByRole("alert").dataset.tone).toBe("error");
      await user.upload(picker, image);
      expect(await screen.findByRole("img", { name: "Hangout cover" })).toBeTruthy();
      expect(upload).toHaveBeenCalledTimes(2);
    });
  });

  describe("and the required fields have been filled", () => {
    beforeEach(async () => {
      await user.type(screen.getByRole("textbox", { name: "Event Name" }), "Open studio");
      await user.type(screen.getByLabelText("Date"), "2099-07-01");
      await user.type(screen.getByLabelText("Start (UTC)"), "18:30");
    });

    it("should return from preview with the draft intact", async () => {
      await user.click(screen.getByRole("button", { name: "Preview" }));
      expect(screen.getByRole("dialog", { name: "Open studio" })).toBeTruthy();
      await user.click(screen.getByRole("button", { name: "Close" }));
      expect(screen.getByRole("textbox", { name: "Event Name" })).toHaveProperty("value", "Open studio");
      expect(submit).not.toHaveBeenCalled();
    });

    it("should update the preview end time and weekly cadence from the edited draft", async () => {
      const endTime = new Intl.DateTimeFormat("en", { hour: "numeric", minute: "2-digit", hour12: true });
      await user.click(screen.getByRole("button", { name: "Preview" }));
      expect(within(screen.getByRole("dialog")).getByText(new RegExp(`– ${endTime.format(new Date("2099-07-01T20:30Z"))}$`))).toBeTruthy();
      expect(within(screen.getByRole("dialog")).queryByText("Repeats weekly")).toBeNull();
      await user.click(screen.getByRole("button", { name: "Close" }));

      const duration = screen.getByRole("spinbutton", { name: "Duration (hours)" });
      await user.clear(duration);
      await user.type(duration, "3");
      await user.click(screen.getByRole("switch", { name: "Repeat Event" }));
      await user.type(screen.getByLabelText("Ends"), "2099-07-29");
      await user.click(screen.getByRole("button", { name: "Preview" }));
      const preview = within(screen.getByRole("dialog"));
      expect(preview.getByText(new RegExp(`^Starting .*– ${endTime.format(new Date("2099-07-01T21:30Z"))}$`))).toBeTruthy();
      expect(preview.getByText("Repeats weekly")).toBeTruthy();
      expect(submit).not.toHaveBeenCalled();
    });

    describe("and the first submission is denied", () => {
      beforeEach(() => {
        submit.mockRejectedValueOnce(new Error("Submission denied"));
      });

      it("should keep the form and let the creator retry", async () => {
        await user.click(screen.getByRole("button", { name: "Submit Event" }));
        await screen.findByText(/Submission denied/);
        expect(screen.getByRole("textbox", { name: "Event Name" })).toHaveProperty("value", "Open studio");
        await user.click(screen.getByRole("button", { name: "Submit Event" }));
        await screen.findByText("Your event was created successfully. You'll be notified once it's approved. Track its status in \"My Events\"");
        expect(submit).toHaveBeenLastCalledWith(expect.objectContaining({
          draft: expect.objectContaining({ name: "Open studio", startDate: "2099-07-01", startTime: "18:30" }),
        }));
      });
    });
  });
});

describe("when the single-page editor receives sign-in without remounting", () => {
  let track: ReturnType<typeof vi.fn<TrackFn>>;
  let page: (signedIn: boolean) => React.JSX.Element;
  beforeEach(() => {
    track = vi.fn<TrackFn>();
    page = (signedIn) => (
      <MemoryRouter initialEntries={["/events/new-event"]}>
        <SubmitHangout
          editor
          signedIn={signedIn}
          onSignIn={vi.fn()}
          trackCtx={{ sid: "test", story: "landings/submit-hangout", variant: "wizard" }}
          categories={[]}
          track={track}
        />
      </MemoryRouter>
    );
  });
  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
  });

  it("should enter the editor and record one start", async () => {
    const view = render(page(false));
    expect(screen.getByRole("button", { name: "Sign In to continue" })).toBeTruthy();
    view.rerender(page(true));
    expect(await screen.findByRole("textbox", { name: "Event Name" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Submit Event" })).toBeTruthy();
    expect(track.mock.calls.filter(([event]) => event === HANGOUT_EVENTS.started)).toHaveLength(1);
  });
});

describe("when an event is saved from the single-page editor", () => {
  let user: ReturnType<typeof userEvent.setup>;
  let submit: ReturnType<typeof vi.fn<SubmitFn>>;
  let router: ReturnType<typeof createMemoryRouter>;
  function page(mode: "create" | "edit") {
    router = createMemoryRouter([
      { path: "/events/new-event", element: <SubmitHangout
            editor
            signedIn
            mode={mode}
            draft={{ ...emptyDraft(), name: "Open studio", startDate: "2099-07-01", startTime: "18:30" }}
            trackCtx={{ sid: "test", story: "landings/submit-hangout", variant: "wizard" }}
            categories={[]}
            submit={submit}
            track={vi.fn()}
          /> },
      { path: "/events", element: <h1>What's On</h1> },
    ], { initialEntries: ["/events/new-event"] });
    return <RouterProvider router={router} />;
  }
  beforeEach(() => {
    user = userEvent.setup();
    submit = vi.fn<SubmitFn>().mockResolvedValue({ id: "saved-event", approved: false });
  });
  afterEach(() => { cleanup(); router.dispose(); document.body.style.overflow = ""; });

  describe("and it was a new event", () => {
    beforeEach(async () => {
      render(page("create"));
      await user.click(screen.getByRole("button", { name: "Submit Event" }));
      await screen.findByRole("button", { name: "My Events" });
    });

    it("should cover the form with the pending-review confirmation and lock the page scroll", () => {
      expect(screen.getByRole("status").textContent).toBe("Your event was created successfully. You'll be notified once it's approved. Track its status in \"My Events\"");
      expect(screen.queryByRole("textbox", { name: "Event Name" })).toBeNull();
      expect(document.body.style.overflow).toBe("hidden");
    });

    it("should move focus to the confirmation message, which leaves the actions outside the live region", () => {
      expect(document.activeElement).toBe(screen.getByRole("status"));
      expect(screen.getByRole("status").contains(screen.getByRole("button", { name: "My Events" }))).toBe(false);
    });

    it("should open My Events from the confirmation", async () => {
      await user.click(screen.getByRole("button", { name: "My Events" }));
      expect(router.state.location.pathname + router.state.location.search).toBe("/events?tab=my");
      expect(document.body.style.overflow).toBe("");
    });

    it("should go back to the Events page from the confirmation", async () => {
      await user.click(screen.getByRole("button", { name: "Back to Explore" }));
      expect(router.state.location.pathname + router.state.location.search).toBe("/events");
    });
  });

  describe("and the submission is still running", () => {
    beforeEach(async () => {
      submit.mockReturnValueOnce(new Promise(() => {}));
      render(page("create"));
      await user.click(screen.getByRole("button", { name: "Submit Event" }));
    });

    it("should show a spinner named for the submission, with no failure", async () => {
      const spinner = await screen.findByRole("progressbar", { name: "Submitting your hangout for review…" });
      expect(spinner.closest("[role=status]")).toBeTruthy();
      expect(screen.queryByRole("alert")).toBeNull();
    });
  });

  describe("and the new event comes back already approved", () => {
    beforeEach(async () => {
      submit.mockResolvedValueOnce({ id: "saved-event", approved: true });
      render(page("create"));
      await user.click(screen.getByRole("button", { name: "Submit Event" }));
    });

    it("should say it is live instead of awaiting review", async () => {
      expect(await screen.findByText("Your event was created and is now live.")).toBeTruthy();
      expect(screen.queryByText(/notified once it's approved/)).toBeNull();
    });
  });

  describe("and it was an existing event", () => {
    beforeEach(async () => {
      render(page("edit"));
      await user.click(screen.getByRole("button", { name: "Save changes" }));
    });

    it("should confirm the update", async () => {
      expect(await screen.findByText("Your event was updated successfully.")).toBeTruthy();
    });
  });

  describe("and the saved event went back to review", () => {
    beforeEach(async () => {
      submit.mockResolvedValueOnce({ id: "saved-event", approved: false, rejected: false });
      render(page("edit"));
      await user.click(screen.getByRole("button", { name: "Save changes" }));
    });

    it("should confirm the update", async () => {
      expect(await screen.findByText("Your event was updated successfully.")).toBeTruthy();
      expect(screen.queryByText(/still rejected/)).toBeNull();
    });
  });

  describe("and the server kept the saved event rejected", () => {
    beforeEach(async () => {
      submit.mockResolvedValueOnce({ id: "saved-event", approved: false, rejected: true });
      render(page("edit"));
      await user.click(screen.getByRole("button", { name: "Save changes" }));
      await screen.findByRole("button", { name: "My Events" });
    });

    it("should say the changes were saved but the event is not back in review", () => {
      expect(screen.getByRole("status").textContent).toBe("Your changes were saved, but the event is still rejected, so it is not back in review.");
      expect(screen.queryByText("Your event was updated successfully.")).toBeNull();
    });

    it("should move focus to the message", () => {
      expect(document.activeElement?.contains(screen.getByRole("status"))).toBe(true);
    });

    it("should offer the way back to My Events", async () => {
      await user.click(screen.getByRole("button", { name: "My Events" }));
      expect(router.state.location.pathname + router.state.location.search).toBe("/events?tab=my");
    });
  });

  describe("and the save fails", () => {
    beforeEach(async () => {
      submit.mockRejectedValueOnce(new Error("Service unavailable"));
      render(page("edit"));
      await user.click(screen.getByRole("button", { name: "Save changes" }));
    });

    it("should keep the form without a confirmation", async () => {
      expect(await screen.findByText(/Service unavailable/)).toBeTruthy();
      expect(screen.queryByRole("button", { name: "My Events" })).toBeNull();
      expect(screen.getByRole("textbox", { name: "Event Name" })).toHaveProperty("value", "Open studio");
    });
  });
});

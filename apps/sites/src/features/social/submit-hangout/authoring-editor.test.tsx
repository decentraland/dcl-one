// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { SubmitFn } from "@/data/lib/catalyst/landings/submit-hangout";
import SubmitHangout from "./SubmitHangout";
import { HANGOUT_EVENTS, type TrackFn } from "./machine";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

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

  afterEach(cleanup);

  it("should block preview and submission while required details are incomplete", () => {
    expect(screen.getByRole<HTMLButtonElement>("button", { name: "Preview" }).disabled).toBe(true);
    expect(screen.getByRole<HTMLButtonElement>("button", { name: "Submit Event" }).disabled).toBe(true);
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

  it("should let the creator retry the same file after an upload failure", async () => {
    upload.mockRejectedValueOnce(new Error("Cover upload failed"));
    const picker = screen.getByLabelText("Upload cover image");
    await user.upload(picker, image);
    expect((await screen.findByRole("alert")).textContent).toBe("Cover upload failed");
    await user.upload(picker, image);
    expect(await screen.findByRole("img", { name: "Hangout cover" })).toBeTruthy();
    expect(upload).toHaveBeenCalledTimes(2);
  });

  describe("and the required fields have been filled", () => {
    beforeEach(async () => {
      await user.type(screen.getByRole("textbox", { name: "Event Name" }), "Open studio");
      fireEvent.change(screen.getByLabelText("Date"), { target: { value: "2099-07-01" } });
      fireEvent.change(screen.getByLabelText("Start (UTC)"), { target: { value: "18:30" } });
    });

    it("should return from preview with the draft intact", async () => {
      await user.click(screen.getByRole("button", { name: "Preview" }));
      expect(screen.getByRole("dialog", { name: "Open studio" })).toBeTruthy();
      await user.click(screen.getByRole("button", { name: "Close" }));
      expect(screen.getByRole("textbox", { name: "Event Name" })).toHaveProperty("value", "Open studio");
      expect(submit).not.toHaveBeenCalled();
    });

    it("should keep the form and let the creator retry after a denied submission", async () => {
      submit.mockRejectedValueOnce(new Error("Submission denied"));
      await user.click(screen.getByRole("button", { name: "Submit Event" }));
      await screen.findByText(/Submission denied/);
      expect(screen.getByRole("textbox", { name: "Event Name" })).toHaveProperty("value", "Open studio");
      await user.click(screen.getByRole("button", { name: "Submit Event" }));
      await screen.findByText(/It is pending review/);
      expect(submit).toHaveBeenLastCalledWith(expect.objectContaining({
        draft: expect.objectContaining({ name: "Open studio", startDate: "2099-07-01", startTime: "18:30" }),
      }));
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
  afterEach(cleanup);

  it("should enter the editor and record one start", async () => {
    const view = render(page(false));
    expect(screen.getByRole("button", { name: "Sign In to continue" })).toBeTruthy();
    view.rerender(page(true));
    expect(await screen.findByRole("textbox", { name: "Event Name" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Submit Event" })).toBeTruthy();
    expect(track.mock.calls.filter(([event]) => event === HANGOUT_EVENTS.started)).toHaveLength(1);
  });
});

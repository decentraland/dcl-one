// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { SubmitFn } from "@/data/lib/catalyst/landings/submit-hangout";
import SubmitHangout from "./SubmitHangout";

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
    expect(screen.getByRole<HTMLButtonElement>("button", { name: "Submit Hangout" }).disabled).toBe(true);
  });

  it("should use the same upload path for dropping a cover", async () => {
    fireEvent.drop(screen.getByText("Upload cover image").closest("label")!, {
      dataTransfer: { files: [image] },
    });
    const cover = await screen.findByRole("img", { name: "Hangout cover" });
    expect(cover.getAttribute("src")).toBe("https://example.test/poster.png");
    expect(upload).toHaveBeenCalledWith(image, false, expect.any(AbortSignal));
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
      await user.type(screen.getByRole("textbox", { name: "Hangout Name" }), "Open studio");
      fireEvent.change(screen.getByLabelText("Date"), { target: { value: "2099-07-01" } });
      fireEvent.change(screen.getByLabelText("Start (UTC)"), { target: { value: "18:30" } });
    });

    it("should return from preview with the draft intact", async () => {
      await user.click(screen.getByRole("button", { name: "Preview" }));
      expect(screen.getByText("Open studio")).toBeTruthy();
      await user.click(screen.getByRole("button", { name: "Back" }));
      expect(screen.getByRole("textbox", { name: "Hangout Name" })).toHaveProperty("value", "Open studio");
      expect(submit).not.toHaveBeenCalled();
    });

    it("should keep the form and let the creator retry after a denied submission", async () => {
      submit.mockRejectedValueOnce(new Error("Submission denied"));
      await user.click(screen.getByRole("button", { name: "Submit Hangout" }));
      await screen.findByText(/Submission denied/);
      expect(screen.getByRole("textbox", { name: "Hangout Name" })).toHaveProperty("value", "Open studio");
      await user.click(screen.getByRole("button", { name: "Submit Hangout" }));
      await screen.findByText(/It is pending review/);
      expect(submit).toHaveBeenLastCalledWith(expect.objectContaining({
        draft: expect.objectContaining({ name: "Open studio", startDate: "2099-07-01", startTime: "18:30" }),
      }));
    });
  });
});

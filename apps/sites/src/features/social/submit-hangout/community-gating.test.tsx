// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { emptyDraft, type HangoutDraft } from "@/data/lib/catalyst/landings/submit-hangout";
import type { HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import SubmitHangout from "./SubmitHangout";

vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });

describe.each(["pending", "failed", "manageable", "not manageable"] as const)("when creating with a preselected community and options are %s", (state) => {
  let draft: HangoutDraft;
  let options: HangoutOptions | null;
  let user: ReturnType<typeof userEvent.setup>;
  beforeEach(() => {
    draft = { ...emptyDraft(), name: "Community hangout", communityId: "preselected" };
    options = state === "pending" ? null : {
      profile: null, profileError: null,
      communities: state === "manageable" ? [{ id: "preselected", name: "Managed community" }] : [],
      communitiesError: state === "failed" ? "Unavailable" : null,
      worlds: [], worldsError: null,
    };
    user = userEvent.setup();
    render(<MemoryRouter initialEntries={["/edit?step=details"]}>
      <SubmitHangout trackCtx={{ sid: "test", story: "landings/submit-hangout", variant: "wizard" }} categories={[]} signedIn draft={draft} options={options} track={vi.fn()} />
    </MemoryRouter>);
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  if (state === "not manageable") {
    it("should block a known unauthorized association while allowing it to be cleared", async () => {
      const alert = screen.getByRole("alert");
      expect(alert.textContent).toBe("Choose a community you manage, or select None.");
      expect(alert.dataset.tone).toBe("error");
      expect((screen.getByRole("button", { name: "Continue" }) as HTMLButtonElement).disabled).toBe(true);
      await user.selectOptions(screen.getByRole("combobox", { name: "Community" }), "");
      expect((screen.getByRole("button", { name: "Continue" }) as HTMLButtonElement).disabled).toBe(false);
    });
  } else {
    it("should let the server validate the association without blocking the wizard", async () => {
      expect((screen.getByRole("button", { name: "Continue" }) as HTMLButtonElement).disabled).toBe(false);
      await user.click(screen.getByRole("button", { name: "Continue" }));
      expect(screen.getByRole("combobox", { name: "Location Type" })).toBeTruthy();
    });
  }
});

describe.each([
  [true, "warning", "Categories couldn't be loaded right now — you can continue and pick one later."],
  [false, "info", "No categories are available right now — you can continue without one."],
] as const)("when the details step has no categories and their read failed is %s", (categoriesError, tone, message) => {
  beforeEach(() => {
    render(<MemoryRouter initialEntries={["/edit?step=details"]}>
      <SubmitHangout trackCtx={{ sid: "test", story: "landings/submit-hangout", variant: "wizard" }} categories={[]} categoriesError={categoriesError} signedIn draft={emptyDraft()} options={null} track={vi.fn()} />
    </MemoryRouter>);
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it(`should explain it in a ${tone} notice`, () => {
    const notice = screen.getByText(message).closest<HTMLElement>("[data-tone]");
    expect(notice?.getAttribute("role")).toBe("status");
    expect(notice?.dataset.tone).toBe(tone);
  });
});

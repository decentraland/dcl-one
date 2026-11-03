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
      schedules: [], schedulesError: null, worlds: [], worldsError: null,
    };
    user = userEvent.setup();
    render(<MemoryRouter initialEntries={["/edit?step=details"]}>
      <SubmitHangout trackCtx={{ sid: "test", story: "landings/submit-hangout", variant: "wizard" }} categories={[]} signedIn draft={draft} options={options} track={vi.fn()} />
    </MemoryRouter>);
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  if (state === "not manageable") {
    it("should block a known unauthorized association while allowing it to be cleared", async () => {
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

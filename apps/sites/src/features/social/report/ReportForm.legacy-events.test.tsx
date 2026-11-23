// @vitest-environment jsdom
import type { ReactNode } from "react";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { legacyAnalyticsReady, track } from "@/core/lib/telemetry/track";
import { dropTrackedUser, restoreTrackedUser } from "@/core/lib/telemetry/tracked-user";
import { ReportSubmitError } from "@/data/lib/catalyst/landings/report";

import ReportForm from "./ReportForm";

// decentraland.org's `Report Player Submit Started` / `Submitted` / `Submit Failed` (decentraland/sites
// `ReportForm.tsx:157-168`), sent next to this form's own `report_*` events.

const auth = vi.hoisted(() => ({ identity: { id: "identity" }, address: "0x1111111111111111111111111111111111111111", isConnected: true }));
const submit = vi.hoisted(() => ({ fn: vi.fn() }));
let ready = true;

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: vi.fn(() => ready),
}));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));
vi.mock("@/data/lib/catalyst/landings/report", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/data/lib/catalyst/landings/report")>()),
  buildSubmitReport: () => submit.fn,
}));
vi.mock("../../components/chrome/SitesLayout", () => ({
  default: ({ children }: { children?: ReactNode }) => <>{children}</>,
}));
vi.mock("@dcl/one-ui/apps/sites/shared/AnimatedBackground/AnimatedBackground", () => ({ AnimatedBackground: () => null }));

const LEGACY = ["Report Player Submit Started", "Report Player Submitted", "Report Player Submit Failed"];

function legacyEvents() {
  return vi.mocked(track).mock.calls.filter(([event]) => LEGACY.includes(event));
}

let user: ReturnType<typeof userEvent.setup>;

function openForm(analyticsUserId?: string) {
  render(
    <MemoryRouter initialEntries={["/report"]}>
      <Routes>
        <Route
          path="/report"
          element={<ReportForm trackCtx={{ sid: "sid-1", story: "landings/report-abuse", variant: "form" }} analyticsUserId={analyticsUserId} />}
        />
        <Route path="/report/success" element={<p>Report sent</p>} />
      </Routes>
    </MemoryRouter>,
  );
}

async function fillValidReport() {
  await user.type(screen.getByRole("textbox", { name: "Reported User Wallet" }), "0x2222222222222222222222222222222222222222");
  await user.selectOptions(screen.getByRole("combobox", { name: "Reason for Report" }), "harassment");
  await user.type(screen.getByRole("textbox", { name: "Description" }), "A test report");
  await user.upload(screen.getByLabelText("Evidence", { selector: "input" }), new File(["image"], "evidence.png", { type: "image/png" }));
  await user.click(screen.getByRole("checkbox", { name: "I confirm this report is accurate." }));
}

beforeEach(() => {
  ready = true;
  user = userEvent.setup();
  submit.fn.mockReset();
});
afterEach(() => {
  cleanup();
  vi.mocked(track).mockReset();
  vi.mocked(legacyAnalyticsReady).mockClear();
  restoreTrackedUser();
});

describe("when a valid report is submitted", () => {
  it("sends Submit Started and Submitted with the chosen reason and the legacy context, then opens the success page", async () => {
    submit.fn.mockResolvedValue({ reportId: "r-1", evidenceKeys: [] });
    openForm("0xabc");
    await fillValidReport();
    await user.click(screen.getByRole("button", { name: "Submit Report" }));
    await screen.findByText("Report sent");
    expect(legacyEvents()).toEqual([
      ["Report Player Submit Started", { reason: "harassment" }, { sid: "sid-1", userId: "0xabc", legacy: true }],
      ["Report Player Submitted", { reason: "harassment" }, { sid: "sid-1", userId: "0xabc", legacy: true }],
    ]);
  });

  it("keeps the form's own events alongside the legacy ones", async () => {
    submit.fn.mockResolvedValue({ reportId: "r-1", evidenceKeys: [] });
    openForm();
    await fillValidReport();
    await user.click(screen.getByRole("button", { name: "Submit Report" }));
    await screen.findByText("Report sent");
    const names = vi.mocked(track).mock.calls.map(([event]) => event);
    expect(names).toEqual(expect.arrayContaining(["report_submit_started", "report_completed"]));
  });

  it("sends Submit Started and Submit Failed with the chosen category when the submission fails", async () => {
    submit.fn.mockRejectedValue(new ReportSubmitError("upload", "Upload failed"));
    openForm();
    await fillValidReport();
    await user.click(screen.getByRole("button", { name: "Submit Report" }));
    await waitFor(() => expect(legacyEvents()).toHaveLength(2));
    expect(legacyEvents()).toEqual([
      ["Report Player Submit Started", { reason: "harassment" }, { sid: "sid-1", legacy: true }],
      ["Report Player Submit Failed", { reason: "harassment" }, { sid: "sid-1", legacy: true }],
    ]);
    expect(vi.mocked(track).mock.calls).toContainEqual(["report_failed", { reason: "upload" }, expect.anything()]);
  });

  it("names no account once a sign-out has started", async () => {
    submit.fn.mockResolvedValue({ reportId: "r-1", evidenceKeys: [] });
    dropTrackedUser();
    openForm("0xabc");
    await fillValidReport();
    await user.click(screen.getByRole("button", { name: "Submit Report" }));
    await screen.findByText("Report sent");
    expect(legacyEvents().map(([, , ctx]) => ctx)).toEqual([
      { sid: "sid-1", legacy: true },
      { sid: "sid-1", legacy: true },
    ]);
  });

  it("sends none of them when the document's analytics was not ready at the press, as isInitialized gated them", async () => {
    ready = false;
    submit.fn.mockResolvedValue({ reportId: "r-1", evidenceKeys: [] });
    openForm();
    await fillValidReport();
    await user.click(screen.getByRole("button", { name: "Submit Report" }));
    await screen.findByText("Report sent");
    expect(legacyEvents()).toEqual([]);
  });
});

describe("when an incomplete report is submitted", () => {
  it("sends no legacy event", async () => {
    openForm();
    await user.click(screen.getByRole("button", { name: "Submit Report" }));
    expect(vi.mocked(track).mock.calls.map(([event]) => event)).toContain("report_validation_failed");
    expect(legacyEvents()).toEqual([]);
    expect(submit.fn).not.toHaveBeenCalled();
  });
});

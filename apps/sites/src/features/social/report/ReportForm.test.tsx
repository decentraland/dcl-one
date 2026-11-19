// @vitest-environment jsdom
import type { ReactNode } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import ReportForm from "./ReportForm";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

vi.mock("@/data/lib/auth/context", () => ({
  useAuth: () => ({ identity: null, address: null, isConnected: false }),
}));
vi.mock("@ui/web/frames/SitesChrome", () => ({
  default: ({ children }: { children?: ReactNode }) => <>{children}</>,
}));
vi.mock("@dcl/one-ui/apps/sites/shared/AnimatedBackground/AnimatedBackground", () => ({ AnimatedBackground: () => null }));

afterEach(() => {
  cleanup();
  sessionStorage.clear();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("when a signed-out visitor fills the report form", () => {
  const returnTo = "/report?reported_address=0x1234567890abcdef1234567890abcdef12345678";
  const description = "The player sent a phishing link";
  const assign = vi.fn();
  let user: ReturnType<typeof userEvent.setup>;

  function openReport(path: string) {
    return render(
      <MemoryRouter initialEntries={[path]}>
        <ReportForm trackCtx={{ sid: "sid", story: "report" } as never} />
      </MemoryRouter>,
    );
  }

  beforeEach(() => {
    assign.mockClear();
    vi.stubGlobal("location", { assign });
    user = userEvent.setup();
  });

  it("should open the sign-in page and restore the draft on return to the same report", async () => {
    const report = openReport(returnTo);
    await user.type(screen.getByRole("textbox", { name: "Description" }), description);
    await user.selectOptions(screen.getByRole("combobox", { name: "Reason for Report" }), "scam_phishing");
    await user.upload(screen.getByLabelText("Evidence", { selector: "input" }), new File(["image"], "evidence.png", { type: "image/png" }));
    await user.click(screen.getByRole("button", { name: "Sign In" }));
    expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=" + encodeURIComponent(returnTo));
    report.unmount();

    const resumed = openReport(returnTo);
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", description);
    expect(screen.getByRole("combobox", { name: "Reason for Report" })).toHaveProperty("value", "scam_phishing");
    expect(screen.getByText("Add your evidence files again after signing in.")).toBeTruthy();
    resumed.unmount();

    openReport(returnTo);
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", "");
  });

  it("should keep a draft out of a report for a different target", async () => {
    const report = openReport(returnTo);
    await user.type(screen.getByRole("textbox", { name: "Description" }), description);
    await user.click(screen.getByRole("button", { name: "Sign In" }));
    report.unmount();

    openReport("/report?reported_address=0x2222222222222222222222222222222222222222");
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", "");
  });

  it("should discard an expired draft on return", async () => {
    const report = openReport(returnTo);
    await user.type(screen.getByRole("textbox", { name: "Description" }), description);
    await user.click(screen.getByRole("button", { name: "Sign In" }));
    report.unmount();
    vi.spyOn(Date, "now").mockReturnValue(Date.now() + 16 * 60 * 1000);

    openReport(returnTo);
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", "");
  });

  it("should still open sign-in when tab storage is unavailable", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("Storage blocked", "SecurityError");
    });
    openReport(returnTo);
    await user.click(screen.getByRole("button", { name: "Sign In" }));
    expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=" + encodeURIComponent(returnTo));
  });
});

describe("when the browser has no crypto.randomUUID", () => {
  let user: ReturnType<typeof userEvent.setup>;
  let input: HTMLElement;
  let first: File;
  let second: File;

  beforeEach(() => {
    vi.stubGlobal("crypto", {});
    user = userEvent.setup();
    first = new File(["first"], "evidence.png", { type: "image/png" });
    second = new File(["second"], "evidence.png", { type: "image/png" });
    render(
      <MemoryRouter>
        <ReportForm trackCtx={{ sid: "sid", story: "report" } as never} />
      </MemoryRouter>,
    );
    input = screen.getByLabelText("Evidence", { selector: "input" });
  });

  it("should keep same-name attachments independent when one is removed", async () => {
    await user.upload(input, first);
    await user.upload(input, second);
    const buttons = screen.getAllByRole("button", {
      name: "Remove evidence.png",
    });
    expect(buttons).toHaveLength(2);
    await user.click(buttons[0]!);
    expect(screen.getByRole("button", { name: "Remove evidence.png" })).toBe(buttons[1]);
  });
});

describe("when evidence files are selected", () => {
  let user: ReturnType<typeof userEvent.setup>;
  let input: HTMLElement;
  let accepted: File[];

  beforeEach(() => {
    user = userEvent.setup({ applyAccept: false });
    accepted = Array.from({ length: 6 }, (_, index) => new File(["image"], `evidence-${index}.png`, { type: "image/png" }));
    render(
      <MemoryRouter>
        <ReportForm trackCtx={{ sid: "sid", story: "report" } as never} />
      </MemoryRouter>,
    );
    input = screen.getByLabelText("Evidence", { selector: "input" });
  });

  describe("and the selection contains files the service rejects", () => {
    let rejected: File[];

    beforeEach(() => {
      rejected = [
        new File(["text"], "notes.txt", { type: "text/plain" }),
        new File([], "empty.png", { type: "image/png" }),
        new File([new Uint8Array(5 * 1024 * 1024 + 1)], "large.png", {
          type: "image/png",
        }),
      ];
    });

    it("should explain each rejection and keep the supported attachment", async () => {
      await user.upload(input, [...rejected, accepted[0]!]);
      expect(screen.getByRole("alert").textContent).toBe(
        "notes.txt is not a supported file type. empty.png is empty. large.png exceeded the 5MB limit.",
      );
      expect(screen.getAllByRole("button", { name: /^Remove / })).toHaveLength(1);
      expect(screen.getByRole("button", { name: "Remove evidence-0.png" })).toBeTruthy();
    });
  });

  describe("and the selection exceeds the available slots", () => {
    it("should cap the list at five and let the visitor reuse a freed slot", async () => {
      await user.upload(input, accepted);
      const add = screen.getByRole("button", { name: "Add File" });
      expect(add).toHaveProperty("disabled", true);
      expect(screen.getByRole("alert").textContent).toBe("Add up to 5 files per report.");
      expect(screen.queryByRole("button", { name: "Remove evidence-5.png" })).toBeNull();
      await user.click(screen.getByRole("button", { name: "Remove evidence-0.png" }));
      expect(add).toHaveProperty("disabled", false);
      await user.upload(input, accepted[5]!);
      expect(screen.getByRole("button", { name: "Remove evidence-5.png" })).toBeTruthy();
      expect(screen.queryByRole("alert")).toBeNull();
    });
  });
});

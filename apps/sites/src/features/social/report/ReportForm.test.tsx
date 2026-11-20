// @vitest-environment jsdom
import type { ReactNode } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Link, MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { safeRedirectTarget } from "../../auth/login/redirect";
import { closeSignIn, installHostSignIn, openSignIn } from "../../components/auth/signin-store";
import ReportForm from "./ReportForm";

const auth = vi.hoisted(() => ({ identity: null, address: null as string | null, isConnected: false }));
vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

vi.mock("@/data/lib/auth/context", () => ({
  useAuth: () => auth,
}));
vi.mock("@ui/web/frames/SitesChrome", () => ({
  default: ({ children }: { children?: ReactNode }) => <>{children}</>,
}));
vi.mock("@dcl/one-ui/apps/sites/shared/AnimatedBackground/AnimatedBackground", () => ({ AnimatedBackground: () => null }));

afterEach(() => {
  cleanup();
  closeSignIn();
  auth.address = null;
  auth.isConnected = false;
  delete (window as { __DCL_SHELL__?: unknown }).__DCL_SHELL__;
  sessionStorage.clear();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("when a signed-out visitor fills the report form", () => {
  const returnTo = "/report?reported_address=0x1234567890abcdef1234567890abcdef12345678#evidence";
  const description = "The player sent a phishing link";
  const assign = vi.fn();
  let user: ReturnType<typeof userEvent.setup>;

  function openReport(path: string) {
    const { pathname, search, hash } = new URL(path, "http://192.0.2.1:5231");
    vi.stubGlobal("location", { pathname, search, hash, assign });
    return render(
      <MemoryRouter initialEntries={[path]}>
        <ReportForm trackCtx={{ sid: "sid", story: "report" } as never} />
        <button type="button" onClick={() => openSignIn()}>
          Header Sign In
        </button>
        <Link to={returnTo}>Original report</Link>
      </MemoryRouter>,
    );
  }

  beforeEach(() => {
    assign.mockClear();
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

    const login = new URL(assign.mock.calls[0]![0] as string, "http://192.0.2.1:5231");
    const resumed = openReport(safeRedirectTarget(login.searchParams.get("redirectTo"), login));
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

    const other = openReport("/report?reported_address=0x2222222222222222222222222222222222222222");
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", "");
    other.unmount();
    openReport(returnTo);
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", description);
  });

  it("should preserve typing during client-side navigation to a saved report URL", async () => {
    const original = openReport(returnTo);
    await user.type(screen.getByRole("textbox", { name: "Description" }), description);
    await user.click(screen.getByRole("button", { name: "Sign In" }));
    original.unmount();

    const other = openReport("/report?reported_address=0x2222222222222222222222222222222222222222");
    await user.type(screen.getByRole("textbox", { name: "Description" }), "A report I started in this view");
    await user.click(screen.getByRole("link", { name: "Original report" }));
    expect(screen.getByRole("textbox", { name: "Reported User Wallet" })).toHaveProperty(
      "value",
      "0x1234567890abcdef1234567890abcdef12345678",
    );
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", "A report I started in this view");
    other.unmount();

    openReport(returnTo);
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", description);
  });

  it("should also restore a draft after using the header sign-in", async () => {
    const report = openReport(returnTo);
    await user.type(screen.getByRole("textbox", { name: "Description" }), description);
    await user.click(screen.getByRole("button", { name: "Header Sign In" }));
    expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=" + encodeURIComponent(returnTo));
    report.unmount();

    openReport(returnTo);
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", description);
  });

  it("should restore a target corrected before sign-in", async () => {
    const path = "/report?reported_address=invalid";
    const report = openReport(path);
    const wallet = screen.getByRole("textbox", { name: "Reported User Wallet" });
    await user.clear(wallet);
    await user.type(wallet, "0x2222222222222222222222222222222222222222");
    await user.click(screen.getByRole("button", { name: "Sign In" }));
    report.unmount();

    openReport(path);
    expect(screen.getByRole("textbox", { name: "Reported User Wallet" })).toHaveProperty(
      "value",
      "0x2222222222222222222222222222222222222222",
    );
  });

  it("should keep the live draft without persisting it for Explorer sign-in", async () => {
    const signIn = vi.fn();
    const uninstall = installHostSignIn(signIn);
    try {
      const report = openReport(returnTo);
      const field = screen.getByRole("textbox", { name: "Description" });
      await user.type(field, description);
      await user.click(screen.getByRole("button", { name: "Sign In" }));
      expect(signIn).toHaveBeenCalledOnce();
      expect(assign).not.toHaveBeenCalled();
      expect(field).toHaveProperty("value", description);
      report.unmount();

      openReport(returnTo);
      expect(screen.getByRole("textbox", { name: "Description" })).toHaveProperty("value", "");
    } finally {
      uninstall();
    }
  });

  it("should keep the live draft without persisting it for desktop shell sign-in", async () => {
    (window as { __DCL_SHELL__?: unknown }).__DCL_SHELL__ = { shell: "tauri" };
    const report = openReport(returnTo);
    const field = screen.getByRole("textbox", { name: "Description" });
    await user.type(field, description);
    await user.click(screen.getByRole("button", { name: "Sign In" }));
    expect(assign).not.toHaveBeenCalled();
    expect(field).toHaveProperty("value", description);
    report.unmount();

    openReport(returnTo);
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

describe("when a signed-in visitor submits an incomplete report", () => {
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    auth.address = "0x1111111111111111111111111111111111111111";
    auth.isConnected = true;
    user = userEvent.setup();
    render(
      <MemoryRouter>
        <ReportForm trackCtx={{ sid: "sid", story: "report" } as never} />
      </MemoryRouter>,
    );
  });

  it("should focus the first invalid field and expose its error as the description", async () => {
    const target = screen.getByRole("textbox", { name: "Reported User Wallet" });
    await user.click(screen.getByRole("button", { name: "Submit Report" }));
    expect(document.activeElement).toBe(target);
    expect(screen.getByRole("textbox", { name: "Reported User Wallet", description: /Please enter a valid wallet address/ })).toBe(target);
  });

  it("should focus Add File and describe the missing evidence after earlier fields are completed", async () => {
    await user.type(screen.getByRole("textbox", { name: "Reported User Wallet" }), "0x2222222222222222222222222222222222222222");
    await user.selectOptions(screen.getByRole("combobox", { name: "Reason for Report" }), "harassment");
    await user.type(screen.getByRole("textbox", { name: "Description" }), "A test report");
    await user.click(screen.getByRole("button", { name: "Submit Report" }));
    const evidence = screen.getByRole("button", { name: "Add File", description: /Please upload the evidence/ });
    expect(document.activeElement).toBe(evidence);
    expect(evidence.hasAttribute("aria-invalid")).toBe(false);
  });
});

describe("when a report link includes an invalid target wallet", () => {
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    auth.address = "0x1111111111111111111111111111111111111111";
    auth.isConnected = true;
    user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/report?reported_address=invalid"]}>
        <ReportForm trackCtx={{ sid: "sid", story: "report" } as never} />
      </MemoryRouter>,
    );
  });

  it("should let the visitor correct the reported wallet after validation fails", async () => {
    const target = screen.getByRole("textbox", { name: "Reported User Wallet" });
    await user.click(screen.getByRole("button", { name: "Submit Report" }));
    expect(document.activeElement).toBe(target);
    await user.clear(target);
    await user.type(target, "0x2222222222222222222222222222222222222222");
    expect(target.getAttribute("aria-invalid")).toBeNull();
  });
});

describe("when the connected wallet does not match the report link", () => {
  let report: ReturnType<typeof render>;
  const entry = "/report?player_address=0x2222222222222222222222222222222222222222";

  function page() {
    return (
      <MemoryRouter initialEntries={[entry]}>
        <ReportForm trackCtx={{ sid: "sid", story: "report" } as never} />
      </MemoryRouter>
    );
  }

  beforeEach(() => {
    report = render(page());
  });

  it("should announce the wallet warning without disturbing focus", () => {
    const description = screen.getByRole("textbox", { name: "Description" });
    description.focus();
    auth.address = "0x1111111111111111111111111111111111111111";
    auth.isConnected = true;
    report.rerender(page());
    expect(screen.getByRole("alert").textContent).toMatch(/The connected wallet does not match the player address/);
    expect(document.activeElement).toBe(description);
    expect(screen.getByRole("button", { name: "Submit Report" })).toHaveProperty("disabled", true);

    auth.address = "0x2222222222222222222222222222222222222222";
    report.rerender(page());
    expect(screen.queryByRole("alert")).toBeNull();
    expect(document.activeElement).toBe(description);
    expect(screen.getByRole("button", { name: "Submit Report" })).toHaveProperty("disabled", false);
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

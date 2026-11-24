// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FALLBACK_PROFILES } from "@web-core/multiplayer-test/rules";
import type { MpReplayOutcome } from "@web-core/multiplayer-test/types";
import { MpReplayDialog } from "@dcl/one-ui";

// The library component is transformed with React Refresh outside a route module; its preamble is stubbed.
vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

const outcome = (over: Partial<MpReplayOutcome> = {}): MpReplayOutcome => ({
  id: null,
  tier: "a",
  hash: "h-1",
  decisionHash: null,
  ...over,
});

describe("MpReplayDialog", () => {
  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
  });

  describe("when the form is submitted with the defaults", () => {
    it("should request an offline replay with the first profile and seed 42", async () => {
      const onSubmit = vi.fn();
      render(<MpReplayDialog runId="r1" profiles={["lan", "3g"]} onSubmit={onSubmit} onClose={vi.fn()} />);
      expect(screen.getByRole("dialog", { name: "Replay run r1" })).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Replay offline" }));
      expect(onSubmit).toHaveBeenCalledWith({ tier: "a", profile: "lan", seed: 42 });
    });
  });

  describe("when the live tier, another profile and a seed are chosen", () => {
    it("should request a live reproduction with them", async () => {
      const onSubmit = vi.fn();
      render(<MpReplayDialog runId="r1" profiles={["lan", "3g"]} onSubmit={onSubmit} onClose={vi.fn()} />);
      await userEvent.click(screen.getByRole("radio", { name: /Reproduce \(live/ }));
      await userEvent.selectOptions(screen.getByRole("combobox", { name: "Network profile" }), "3g");
      fireEvent.change(screen.getByRole("spinbutton", { name: "Seed" }), { target: { value: "7" } });
      await userEvent.click(screen.getByRole("button", { name: "Reproduce live" }));
      expect(onSubmit).toHaveBeenCalledWith({ tier: "b", profile: "3g", seed: 7 });
    });
  });

  describe("when the seed is cleared", () => {
    it("should send seed 0", async () => {
      const onSubmit = vi.fn();
      render(<MpReplayDialog runId="r1" profiles={["lan"]} onSubmit={onSubmit} onClose={vi.fn()} />);
      fireEvent.change(screen.getByRole("spinbutton", { name: "Seed" }), { target: { value: "" } });
      await userEvent.click(screen.getByRole("button", { name: "Replay offline" }));
      expect(onSubmit).toHaveBeenCalledWith({ tier: "a", profile: "lan", seed: 0 });
    });
  });

  describe("when the sidecar lists no profiles", () => {
    it("should offer the fallback profiles", () => {
      render(<MpReplayDialog runId="r1" profiles={[]} onSubmit={vi.fn()} onClose={vi.fn()} />);
      expect(screen.getByRole("combobox", { name: "Network profile" })).toHaveValue(FALLBACK_PROFILES[0]);
      expect(screen.getAllByRole("option")).toHaveLength(FALLBACK_PROFILES.length);
    });
  });

  // jsdom has no implicit submission for a button outside the form, so this checks what the
  // browser's Enter-to-submit relies on: the action is the form's submit button.
  describe("when the form is submitted through its action outside it", () => {
    it("should belong to the fields' form and send their values", () => {
      const onSubmit = vi.fn();
      render(<MpReplayDialog runId="r1" profiles={["lan"]} onSubmit={onSubmit} onClose={vi.fn()} />);
      const seed = screen.getByRole("spinbutton", { name: "Seed" }) as HTMLInputElement;
      const submit = screen.getByRole("button", { name: "Replay offline" }) as HTMLButtonElement;
      fireEvent.change(seed, { target: { value: "9" } });
      expect(submit.form).toBe(seed.form);
      seed.form!.requestSubmit(submit);
      expect(onSubmit).toHaveBeenCalledWith({ tier: "a", profile: "lan", seed: 9 });
    });
  });

  describe("when Cancel is pressed with no request in flight", () => {
    it("should close the dialog", async () => {
      const onClose = vi.fn();
      render(<MpReplayDialog runId="r1" profiles={["lan"]} onSubmit={vi.fn()} onClose={onClose} />);
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
      expect(onClose).toHaveBeenCalledTimes(1);
    });
  });

  describe("when the backdrop is pressed", () => {
    it("should stay open and keep the chosen tier, profile and seed", async () => {
      const onSubmit = vi.fn();
      const onClose = vi.fn();
      render(<MpReplayDialog runId="r1" profiles={["lan", "3g"]} onSubmit={onSubmit} onClose={onClose} />);
      await userEvent.click(screen.getByRole("radio", { name: /Reproduce \(live/ }));
      await userEvent.selectOptions(screen.getByRole("combobox", { name: "Network profile" }), "3g");
      fireEvent.change(screen.getByRole("spinbutton", { name: "Seed" }), { target: { value: "7" } });
      const dialog = screen.getByRole("dialog");
      fireEvent.pointerDown(dialog, { clientX: -10, clientY: -10 });
      fireEvent.click(dialog, { clientX: -10, clientY: -10 });
      expect(onClose).not.toHaveBeenCalled();
      await userEvent.click(screen.getByRole("button", { name: "Reproduce live" }));
      expect(onSubmit).toHaveBeenCalledWith({ tier: "b", profile: "3g", seed: 7 });
    });
  });

  describe("when a request is in flight", () => {
    it("should mark the submit action busy and refuse a second submit", async () => {
      const onSubmit = vi.fn();
      render(<MpReplayDialog runId="r1" profiles={["lan"]} requesting onSubmit={onSubmit} onClose={vi.fn()} />);
      const submit = screen.getByRole("button", { name: "Replay offline" });
      expect(submit).toHaveAttribute("aria-busy", "true");
      expect(submit).toHaveAttribute("aria-disabled", "true");
      await userEvent.click(submit);
      fireEvent.submit(screen.getByRole("radiogroup").closest("form")!);
      expect(onSubmit).not.toHaveBeenCalled();
    });

    it("should still close from Cancel and Escape, which abandons the request", async () => {
      const onClose = vi.fn();
      render(<MpReplayDialog runId="r1" profiles={["lan"]} requesting onSubmit={vi.fn()} onClose={onClose} />);
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
      fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
      expect(onClose).toHaveBeenCalledTimes(2);
    });
  });

  describe("when the outcome replaces the form", () => {
    it("should move focus to the Close action", () => {
      const props = { runId: "r1", profiles: ["lan"], onSubmit: vi.fn(), onClose: vi.fn() };
      const { rerender } = render(<MpReplayDialog {...props} requesting />);
      screen.getByRole("button", { name: "Replay offline" }).focus();
      rerender(<MpReplayDialog {...props} outcome={outcome()} />);
      const close = screen.getAllByRole("button", { name: "Close" }).find((b) => b.dataset.mpAction === "close-replay");
      expect(close).toHaveFocus();
    });
  });

  describe("when the replay request failed", () => {
    it("should announce the error and keep the form", () => {
      render(
        <MpReplayDialog runId="r1" profiles={["lan"]} error="Refused (409)" onSubmit={vi.fn()} onClose={vi.fn()} />,
      );
      expect(screen.getByRole("alert")).toHaveTextContent("Refused (409)");
      expect(screen.getByRole("button", { name: "Replay offline" })).toBeEnabled();
    });
  });

  describe("when an offline replay finished", () => {
    it("should show the hashes and close from the Close action", async () => {
      const onClose = vi.fn();
      render(
        <MpReplayDialog
          runId="r1"
          profiles={["lan"]}
          outcome={outcome({ decisionHash: "d-1" })}
          onSubmit={vi.fn()}
          onClose={onClose}
        />,
      );
      expect(screen.getByText("Replay (exact, offline) finished")).toBeInTheDocument();
      expect(screen.getByText("h-1")).toBeInTheDocument();
      expect(screen.getByText("d-1")).toBeInTheDocument();
      expect(screen.queryByRole("radiogroup")).toBeNull();
      const actions = screen.getAllByRole("button", { name: "Close" });
      expect(actions).toHaveLength(2); // the corner button and the action
      await userEvent.click(actions.find((button) => button.dataset.mpAction === "close-replay")!);
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("should show a dash when the sidecar returned no hash", () => {
      render(
        <MpReplayDialog
          runId="r1"
          profiles={["lan"]}
          outcome={outcome({ hash: null })}
          onSubmit={vi.fn()}
          onClose={vi.fn()}
        />,
      );
      expect(screen.getByText("\u{2014}")).toBeInTheDocument();
      expect(screen.queryByText("Decision-log hash")).toBeNull();
    });
  });

  describe("when a live reproduction started", () => {
    it("should name the new run", () => {
      render(
        <MpReplayDialog
          runId="r1"
          profiles={["lan"]}
          outcome={outcome({ tier: "b", id: "r2", hash: null })}
          onSubmit={vi.fn()}
          onClose={vi.fn()}
        />,
      );
      expect(screen.getByText("Reproduce (live, same conditions) started")).toBeInTheDocument();
      expect(screen.getByText("r2")).toBeInTheDocument();
    });
  });
});

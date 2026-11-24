// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

import { Dialog } from "@dcl/one-ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function Harness({ keepOpener = true }: { keepOpener?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      {keepOpener || !open ? (
        <button type="button" onClick={() => setOpen(true)}>
          Open
        </button>
      ) : null}
      {open ? (
        <Dialog labelledBy="t" onClose={() => setOpen(false)}>
          <h2 id="t">Details</h2>
          <button type="button">First</button>
          <button type="button" hidden>
            Hidden
          </button>
          <div inert>
            <button type="button">Inert</button>
          </div>
          <details>
            <summary>More</summary>
          </details>
        </Dialog>
      ) : null}
    </>
  );
}

describe("Dialog", () => {
  it("should cycle Tab through visible controls only, summary included, and skip hidden and inert ones", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.tab();
    expect(document.activeElement?.textContent).toBe("First");
    await user.tab();
    expect(document.activeElement?.textContent).toBe("More");
    await user.tab();
    expect(document.activeElement?.textContent).toBe("First");
    await user.tab({ shift: true });
    expect(document.activeElement?.textContent).toBe("More");
  });

  it("should close on Escape and give focus back to the opener", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement?.textContent).toBe("Open");
  });

  it("should not try to focus an opener that left the page", async () => {
    const user = userEvent.setup();
    render(<Harness keepOpener={false} />);
    const opener = screen.getByRole("button", { name: "Open" });
    const focus = vi.spyOn(opener, "focus");
    await user.click(opener);
    focus.mockClear();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(focus).not.toHaveBeenCalled();
  });

  it("should cap a narrower dialog at its own width", () => {
    render(<Dialog labelledBy="t" maxWidth={540} onClose={() => {}}><h2 id="t">Delete</h2></Dialog>);
    expect(screen.getByRole("dialog").style.getPropertyValue("--dialog-max-width")).toBe("540px");
  });

  it("should keep the default width when none is given", () => {
    render(<Dialog labelledBy="t" onClose={() => {}}><h2 id="t">Details</h2></Dialog>);
    expect(screen.getByRole("dialog").getAttribute("style")).toBeNull();
  });
});

import { useState } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Toast, ToastHost, createToastStore, type ToastCloseReason, type ToastProps } from "./Toast";

// The time to close is the subject: the clock is fake, and it still follows real time so the waits of
// Testing Library end. The margins in the cases are wider than a test takes.
beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const setupUser = () => userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
const wait = (ms: number) => act(() => { vi.advanceTimersByTime(ms); });

function Harness({ onClose, ...props }: Partial<ToastProps> & { onClose?: (reason: ToastCloseReason) => void }) {
  const [open, setOpen] = useState(true);
  return (
    <>
      <button>Elsewhere</button>
      <Toast open={open} severity="success" onClose={(reason) => { onClose?.(reason); setOpen(false); }} {...props}>
        The scene was published
      </Toast>
    </>
  );
}

describe("Toast", () => {
  test("a toast with no severity is a polite live region outside the tree that rendered it", () => {
    const { container } = render(<Toast><span>Copied</span></Toast>);

    const toast = screen.getByRole("status");
    expect(toast).toHaveTextContent("Copied");
    expect(container.contains(toast)).toBe(false);
  });

  test.each([
    ["error", "alert"],
    ["warning", "alert"],
    ["info", "status"],
    ["success", "status"],
  ] as const)("a toast of severity %s is one live region with the role %s", (severity, role) => {
    render(<Toast severity={severity}>The deploy ended</Toast>);

    expect(screen.getAllByRole(role)).toHaveLength(1);
    expect(screen.getByRole(role)).toHaveTextContent("The deploy ended");
    expect(screen.queryByRole(role === "alert" ? "status" : "alert")).toBeNull();
  });

  test("the caller can replace the role, with and without a severity", () => {
    const { rerender } = render(<Toast role="alert">Connection lost</Toast>);
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.queryByRole("status")).toBeNull();

    rerender(<Toast severity="error" role="status">Connection lost</Toast>);
    expect(screen.getAllByRole("status")).toHaveLength(1);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  test("a closed toast renders nothing", () => {
    render(<Toast open={false} severity="info">Hidden</Toast>);

    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByText("Hidden")).toBeNull();
  });

  test("the title and the controls of the caller are in the toast", () => {
    render(<Toast severity="info" title="New version" action={<button>Restart</button>}>Restart to update.</Toast>);

    expect(screen.getByRole("status")).toHaveTextContent("New versionRestart to update.");
    expect(screen.getByRole("status")).toContainElement(screen.getByRole("button", { name: "Restart" }));
  });

  test("a toast asks to close after its duration, and not before", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={5000} />);

    wait(4000);
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("status")).toBeInTheDocument();

    wait(1500);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("timeout");
    expect(screen.queryByRole("status")).toBeNull();
  });

  test("a toast with no duration stays until it is dismissed", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={null} />);

    wait(60_000);

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("status")).toBeInTheDocument();
  });

  test("the time to close stops while the pointer is on the toast and goes on when it leaves", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={5000} />);
    wait(3000);

    await user.hover(screen.getByRole("status"));
    wait(20_000);
    expect(onClose).not.toHaveBeenCalled();

    await user.unhover(screen.getByRole("status"));
    wait(1000);
    expect(onClose).not.toHaveBeenCalled();
    wait(1500);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("timeout");
  });

  test("the time to close stops while focus is in the toast and goes on when focus leaves", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={5000} />);
    wait(3000);

    await user.tab();
    await user.tab();
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
    wait(20_000);
    expect(onClose).not.toHaveBeenCalled();

    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
    wait(1000);
    expect(onClose).not.toHaveBeenCalled();
    wait(1500);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test("focus that moves between two controls of the toast keeps the time stopped", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(
      <Toast duration={5000} onClose={onClose}>
        <button>Undo</button>
        <button>Details</button>
      </Toast>,
    );

    await user.tab();
    expect(screen.getByRole("button", { name: "Undo" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Details" })).toHaveFocus();
    wait(20_000);

    expect(onClose).not.toHaveBeenCalled();
  });

  test("the time stays stopped while either the pointer or focus is on the toast", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={5000} />);

    await user.hover(screen.getByRole("status"));
    await user.tab();
    await user.tab();
    await user.unhover(screen.getByRole("status"));
    wait(20_000);
    expect(onClose).not.toHaveBeenCalled();

    await user.tab({ shift: true });
    wait(5500);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test("the close button has a name and dismisses the toast", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />);

    await user.click(screen.getByRole("button", { name: "Close" }));

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("dismiss");
    expect(screen.queryByRole("status")).toBeNull();
  });

  test("the caller can name the close button", () => {
    render(<Harness closeLabel="Dismiss the message" />);

    expect(screen.getByRole("button", { name: "Dismiss the message" })).toBeInTheDocument();
  });

  test("Escape in the toast closes it", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />);
    await user.tab();
    await user.tab();
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();

    await user.keyboard("{Escape}");

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("escape");
  });

  test("Escape outside the toast is not for the toast", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />);
    await user.tab();
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();

    await user.keyboard("{Escape}");

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("status")).toBeInTheDocument();
  });

  test("Escape that another handler took for itself does not close the toast", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    const take = (event: KeyboardEvent) => event.preventDefault();
    document.addEventListener("keydown", take, true);
    render(<Harness onClose={onClose} />);
    await user.tab();
    await user.tab();

    await user.keyboard("{Escape}");
    document.removeEventListener("keydown", take, true);

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("status")).toBeInTheDocument();
  });

  test("a toast set to hold on window blur stops the time while the window is not the active one", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={5000} holdOnWindowBlur />);
    wait(3000);

    fireEvent.blur(window);
    wait(20_000);
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.focus(window);
    wait(1000);
    expect(onClose).not.toHaveBeenCalled();
    wait(1500);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("timeout");
  });

  test("a toast that does not hold on window blur goes on while the window is not the active one", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={5000} />);
    wait(3000);

    fireEvent.blur(window);
    wait(2500);

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("timeout");
  });

  test("the time stays stopped while the pointer is on the toast when the window is active again", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={5000} holdOnWindowBlur />);

    await user.hover(screen.getByRole("status"));
    fireEvent.blur(window);
    fireEvent.focus(window);
    wait(20_000);

    expect(onClose).not.toHaveBeenCalled();
  });

  test("a toast with a resume duration waits that long after a stop ends, with little time left", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={5000} resumeDuration={2500} />);
    wait(4500);

    await user.hover(screen.getByRole("status"));
    wait(20_000);
    await user.unhover(screen.getByRole("status"));
    wait(2000);
    expect(onClose).not.toHaveBeenCalled();
    wait(1000);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("timeout");
  });

  test("a toast with a resume duration waits that long after a stop ends, with much time left", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} duration={60_000} resumeDuration={2500} />);
    wait(1000);

    await user.hover(screen.getByRole("status"));
    await user.unhover(screen.getByRole("status"));
    wait(2000);
    expect(onClose).not.toHaveBeenCalled();
    wait(1000);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test("a toast that closed with the pointer on it starts its time again when it opens again", async () => {
    const user = setupUser();
    const onClose = vi.fn();
    const { rerender } = render(<Toast open duration={1000} onClose={onClose}>Copied</Toast>);
    await user.hover(screen.getByRole("status"));

    rerender(<Toast open={false} duration={1000} onClose={onClose}>Copied</Toast>);
    rerender(<Toast open duration={1000} onClose={onClose}>Copied</Toast>);
    wait(1500);

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("timeout");
  });

  test("a toast that closed does not ask to close again", () => {
    const onClose = vi.fn();
    const { rerender } = render(<Toast open duration={1000} onClose={onClose}>Copied</Toast>);

    rerender(<Toast open={false} duration={1000} onClose={onClose}>Copied</Toast>);
    wait(5000);

    expect(onClose).not.toHaveBeenCalled();
  });

  test("a new handler from the parent does not start the time again", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = render(<Toast duration={5000} onClose={first}>Copied</Toast>);
    wait(4000);

    rerender(<Toast duration={5000} onClose={second}>Copied</Toast>);
    wait(1500);

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith("timeout");
  });
});

describe("ToastHost", () => {
  test("a host shows the toasts of its store, each with a close button", () => {
    const store = createToastStore();
    render(<ToastHost store={store} />);
    expect(screen.queryByRole("status")).toBeNull();

    act(() => {
      store.show({ severity: "success", children: "Scene published" });
      store.show({ severity: "error", children: "Deploy failed" });
    });

    expect(screen.getByRole("status")).toHaveTextContent("Scene published");
    expect(screen.getByRole("alert")).toHaveTextContent("Deploy failed");
    expect(screen.getAllByRole("button", { name: "Close" })).toHaveLength(2);
  });

  test("a toast that reaches its duration leaves the store and tells its own handler", () => {
    const store = createToastStore();
    const onClose = vi.fn();
    render(<ToastHost store={store} />);
    act(() => { store.show({ severity: "info", duration: 2000, onClose, children: "Saved" }); });

    wait(2500);

    expect(onClose).toHaveBeenCalledWith("timeout");
    expect(store.get()).toEqual([]);
    expect(screen.queryByRole("status")).toBeNull();
  });

  test("the close button removes only its toast", async () => {
    const user = setupUser();
    const store = createToastStore();
    render(<ToastHost store={store} />);
    act(() => {
      store.show({ severity: "info", duration: null, closeLabel: "Close the first", children: "First" });
      store.show({ severity: "info", duration: null, closeLabel: "Close the second", children: "Second" });
    });

    await user.click(screen.getByRole("button", { name: "Close the first" }));

    expect(screen.queryByText("First")).toBeNull();
    expect(screen.getByText("Second")).toBeInTheDocument();
    expect(store.get().map((item) => item.children)).toEqual(["Second"]);
  });

  test("a new toast does not start the time of the ones already shown again", () => {
    const store = createToastStore();
    render(<ToastHost store={store} />);
    act(() => { store.show({ severity: "info", duration: 5000, children: "First" }); });
    wait(4000);

    act(() => { store.show({ severity: "info", duration: 5000, children: "Second" }); });
    wait(1500);

    expect(screen.queryByText("First")).toBeNull();
    expect(screen.getByText("Second")).toBeInTheDocument();
  });

  test("the pointer on one toast stops only the time of that toast", async () => {
    const user = setupUser();
    const store = createToastStore();
    render(<ToastHost store={store} />);
    act(() => {
      store.show({ severity: "info", duration: 3000, children: "First" });
      store.show({ severity: "info", duration: 3000, children: "Second" });
    });

    await user.hover(screen.getByText("First"));
    wait(3500);

    expect(screen.getByText("First")).toBeInTheDocument();
    expect(screen.queryByText("Second")).toBeNull();
  });

  test("code outside the tree dismisses a toast by its id", () => {
    const store = createToastStore();
    render(<ToastHost store={store} />);
    let id = "";
    act(() => { id = store.show({ severity: "info", duration: null, children: "Uploading" }); });
    expect(screen.getByText("Uploading")).toBeInTheDocument();

    act(() => { store.dismiss(id); });

    expect(screen.queryByText("Uploading")).toBeNull();
  });

  test("a host shows the toasts the app renders in it before the ones of the store", () => {
    const store = createToastStore();
    render(<ToastHost store={store}><Toast severity="info" duration={null}>From the app</Toast></ToastHost>);

    act(() => { store.show({ severity: "info", duration: null, children: "From the store" }); });

    expect(screen.getAllByRole("status").map((node) => node.textContent)).toEqual(["From the app", "From the store"]);
  });

  test("a host with no store shows only its children", () => {
    render(<ToastHost><Toast duration={null}>From the app</Toast></ToastHost>);

    expect(screen.getByRole("status")).toHaveTextContent("From the app");
  });

  test("a host that leaves the page takes its toasts with it and stops listening to the store", () => {
    const store = createToastStore();
    const { unmount } = render(<ToastHost store={store} />);
    act(() => { store.show({ severity: "info", duration: 1000, children: "Saved" }); });

    unmount();
    wait(5000);

    expect(screen.queryByText("Saved")).toBeNull();
    expect(store.get()).toHaveLength(1);
  });
});

describe("createToastStore", () => {
  test("show adds a toast at the end and returns a new id each time", () => {
    const store = createToastStore();

    const first = store.show({ children: "First" });
    const second = store.show({ children: "Second" });

    expect(first).not.toBe(second);
    expect(store.get().map((item) => [item.id, item.children])).toEqual([[first, "First"], [second, "Second"]]);
  });

  test("show with an id in use replaces that toast and moves it to the end", () => {
    const store = createToastStore();
    store.show({ id: "upload", children: "Uploading" });
    store.show({ children: "Other" });

    const id = store.show({ id: "upload", children: "Uploaded" });

    expect(id).toBe("upload");
    expect(store.get().map((item) => item.children)).toEqual(["Other", "Uploaded"]);
  });

  test("dismiss removes the toast with that id", () => {
    const store = createToastStore();
    const first = store.show({ children: "First" });
    store.show({ children: "Second" });

    store.dismiss(first);

    expect(store.get().map((item) => item.children)).toEqual(["Second"]);
  });

  test("a listener hears each change, and not a dismissal of an id the store does not hold", () => {
    const store = createToastStore();
    const listener = vi.fn();
    store.subscribe(listener);

    const id = store.show({ children: "First" });
    expect(listener).toHaveBeenCalledTimes(1);
    store.dismiss("unknown");
    expect(listener).toHaveBeenCalledTimes(1);
    store.dismiss(id);
    expect(listener).toHaveBeenCalledTimes(2);
  });

  test("a listener that left hears nothing more, and the others still do", () => {
    const store = createToastStore();
    const gone = vi.fn();
    const stays = vi.fn();
    const leave = store.subscribe(gone);
    store.subscribe(stays);

    leave();
    store.show({ children: "First" });

    expect(gone).not.toHaveBeenCalled();
    expect(stays).toHaveBeenCalledTimes(1);
  });

  test("the list keeps its identity until it changes", () => {
    const store = createToastStore();
    const empty = store.get();
    expect(store.get()).toBe(empty);

    store.show({ children: "First" });

    expect(store.get()).not.toBe(empty);
    expect(store.get()).toBe(store.get());
  });

  test("two stores share nothing", () => {
    const one = createToastStore();
    const two = createToastStore();

    one.show({ children: "First" });

    expect(two.get()).toEqual([]);
  });
});

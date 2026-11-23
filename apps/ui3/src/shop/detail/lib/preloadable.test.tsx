import { act, render, screen } from "@testing-library/react";
import { Suspense, useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { DialogBoundary, preloadable } from "./preloadable";

function Hello({ name }: { name: string }) {
  return <p>hello {name}</p>;
}

describe("preloadable", () => {
  it("loads on the first render when nothing preloaded it", async () => {
    const load = vi.fn(async () => ({ default: Hello }));
    const Part = preloadable(load);
    render(
      <Suspense fallback={<p>loading</p>}>
        <Part name="a" />
      </Suspense>,
    );
    expect(screen.getByText("loading")).toBeTruthy();
    expect(await screen.findByText("hello a")).toBeTruthy();
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("renders at once, without the fallback, once preloaded", async () => {
    const Part = preloadable(async () => ({ default: Hello }));
    await act(() => Part.preload());
    render(
      <Suspense fallback={<p>loading</p>}>
        <Part name="b" />
      </Suspense>,
    );
    expect(screen.queryByText("loading")).toBeNull();
    expect(screen.getByText("hello b")).toBeTruthy();
  });

  it("keeps a part mounted through the lazy wrapper when its code arrives", async () => {
    const Part = preloadable(async () => ({ default: Hello }));
    const view = render(
      <Suspense fallback={<p>loading</p>}>
        <Part name="c" />
      </Suspense>,
    );
    const first = await screen.findByText("hello c");
    view.rerender(
      <Suspense fallback={<p>loading</p>}>
        <Part name="c" />
      </Suspense>,
    );
    expect(screen.getByText("hello c")).toBe(first);
  });

  it("does not keep a failed load: the preload settles, the failed mount closes through its boundary, the next one loads again", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    let fails = true;
    const load = vi.fn(async () => {
      if (fails) throw new Error("chunk");
      return { default: Hello };
    });
    const Part = preloadable(load);
    await expect(Part.preload()).resolves.toBeUndefined();

    function Page() {
      const [open, setOpen] = useState(true);
      return (
        <>
          <button onClick={() => setOpen(true)}>open</button>
          <DialogBoundary onClose={() => setOpen(false)}>
            <Suspense fallback={<p>loading</p>}>{open ? <Part name="d" /> : null}</Suspense>
          </DialogBoundary>
        </>
      );
    }
    render(<Page />);
    // Every load fails: the dialog closes and the page stays.
    await vi.waitFor(() => expect(screen.queryByText("loading")).toBeNull());
    expect(screen.queryByText("hello d")).toBeNull();
    expect(screen.getByText("open")).toBeTruthy();

    // The code is reachable again: the next open loads it instead of rethrowing the kept failure.
    fails = false;
    const before = load.mock.calls.length;
    await act(async () => screen.getByText("open").click());
    expect(await screen.findByText("hello d")).toBeTruthy();
    expect(load.mock.calls.length).toBeGreaterThan(before);
  });
});

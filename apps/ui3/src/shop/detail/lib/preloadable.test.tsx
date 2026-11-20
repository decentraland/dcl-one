import { act, render, screen } from "@testing-library/react";
import { Suspense } from "react";
import { describe, expect, it, vi } from "vitest";

import { preloadable } from "./preloadable";

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
});

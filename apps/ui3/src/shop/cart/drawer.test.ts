import { describe, expect, it } from "vitest";

import { createCartDrawer } from "./drawer";

describe("the cart drawer's state, as upstream's store/cart.ts", () => {
  it("an add opens it with a banner that counts the units added while it stays open", () => {
    const drawer = createCartDrawer();
    expect(drawer.getSnapshot()).toEqual({ open: false, justAdded: 0 });
    drawer.added();
    expect(drawer.getSnapshot()).toEqual({ open: true, justAdded: 1 });
    drawer.added();
    expect(drawer.getSnapshot()).toEqual({ open: true, justAdded: 2 });
  });

  it("closing forgets the banner, so the next add counts from one", () => {
    const drawer = createCartDrawer();
    drawer.added();
    drawer.added();
    drawer.close();
    expect(drawer.getSnapshot()).toEqual({ open: false, justAdded: 0 });
    drawer.added();
    expect(drawer.getSnapshot().justAdded).toBe(1);
  });

  it("the cart icon opens it with no banner, and keeps one an add already showed", () => {
    const drawer = createCartDrawer();
    drawer.open();
    expect(drawer.getSnapshot()).toEqual({ open: true, justAdded: 0 });
    drawer.close();
    drawer.added();
    drawer.open();
    expect(drawer.getSnapshot()).toEqual({ open: true, justAdded: 1 });
  });

  it("a removed line takes the banner with it, and the server render is always closed", () => {
    const drawer = createCartDrawer();
    drawer.added();
    drawer.clearBanner();
    expect(drawer.getSnapshot()).toEqual({ open: true, justAdded: 0 });
    expect(drawer.getServerSnapshot()).toEqual({ open: false, justAdded: 0 });
  });

  it("tells its readers only when something changed", () => {
    const drawer = createCartDrawer();
    let calls = 0;
    const stop = drawer.subscribe(() => calls++);
    drawer.close();
    drawer.clearBanner();
    expect(calls).toBe(0);
    drawer.added();
    expect(calls).toBe(1);
    stop();
    drawer.close();
    expect(calls).toBe(1);
  });
});

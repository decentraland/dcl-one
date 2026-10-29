import { act, type ReactNode } from "react";
import { hydrateRoot } from "react-dom/client";
import { renderToString } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { saveReceipt } from "./history/receipt";
import SuccessPage from "./history/SuccessPage";
import { ShopHarness } from "./land/testkit-react";
import Rankings from "./market/pages/Rankings";
import NamesHub from "./names/NamesHub";
import { RESUME_NAME_KEY } from "./names/resume";

const original = window.matchMedia;
const setMatches = (matches: boolean) => {
  window.matchMedia = ((media: string) => ({ matches, media, addEventListener() {}, removeEventListener() {} })) as unknown as typeof window.matchMedia;
};

async function hydrate(server: ReactNode, client: ReactNode, browser: () => void) {
  setMatches(false);
  const html = renderToString(server);
  browser();
  const container = document.createElement("div");
  container.innerHTML = html;
  document.body.appendChild(container);
  const errors: unknown[] = [];
  const root = await act(async () => hydrateRoot(container, client, { onRecoverableError: (error) => errors.push(error) }));
  return { container, errors, unmount: () => act(() => root.unmount()) };
}

beforeEach(() => {
  sessionStorage.clear();
  vi.stubGlobal("fetch", () => new Promise(() => {}));
});
afterEach(() => {
  window.matchMedia = original;
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

describe("Shop pages hydrate what the server rendered", () => {
  const names = (path = "/shop/names") => (
    <ShopHarness session="signedOut" path={path}>
      <NamesHub loadBanned={async () => []} />
    </ShopHarness>
  );

  it("/shop/names under reduced motion keeps the static placeholder", async () => {
    const page = await hydrate(names(), names(), () => setMatches(true));
    expect(page.errors).toEqual([]);
    expect(page.container.querySelector("input")?.placeholder).toBe("yourname");
    page.unmount();
  });

  it("/shop/names resumes a stashed NAME after hydrating", async () => {
    const page = await hydrate(names(), names(), () => sessionStorage.setItem(RESUME_NAME_KEY, "Nacho"));
    expect(page.errors).toEqual([]);
    expect(page.container.querySelector("input")?.value).toBe("Nacho");
    expect(sessionStorage.getItem(RESUME_NAME_KEY)).toBeNull();
    page.unmount();
  });

  const rankings = (path = "/shop/rankings") => (
    <ShopHarness session="signedOut" path={path}>
      <Rankings />
    </ShopHarness>
  );

  it("/shop/rankings on a phone switches to the list after hydrating", async () => {
    const page = await hydrate(rankings(), rankings(), () => setMatches(true));
    expect(page.errors).toEqual([]);
    expect(page.container.querySelector(".shop-rk-dropdown--entity")).not.toBeNull();
    page.unmount();
  });

  it("/shop/rankings applies a #rankings- deep link after hydrating", async () => {
    const page = await hydrate(rankings(), rankings("/shop/rankings#rankings-creators-week-most_volume"), () => {});
    expect(page.errors).toEqual([]);
    expect(page.container.querySelector(".shop-rk-tab.is-active")?.textContent).toBe("Creators");
    page.unmount();
  });

  const success = () => (
    <ShopHarness session="signedOut" path="/shop/success">
      <SuccessPage />
    </ShopHarness>
  );
  const receipt = { items: [{ name: "Rainbow Hat", quantity: 1, method: "credits" as const }], txs: [], settled: true };

  it("/shop/success shows the stored receipt after hydrating", async () => {
    const page = await hydrate(success(), success(), () => saveReceipt(receipt));
    expect(page.errors).toEqual([]);
    expect(page.container.querySelector("[data-testid=success-done]")?.textContent).toContain("Rainbow Hat");
    page.unmount();
  });

  it("/shop/success plays its confetti only after hydrating", async () => {
    const page = await hydrate(success(), success(), () => saveReceipt(receipt));
    expect(page.errors).toEqual([]);
    expect(page.container.querySelector("[data-testid=confetti]")).not.toBeNull();
    page.unmount();
  });
});

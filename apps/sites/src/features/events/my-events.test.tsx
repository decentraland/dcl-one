// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { invalidateEventsAfterWrite } from "@/data/lib/catalyst/places/events-page";
import { usePrivateQueryLifecycle } from "../components/chrome/usePrivateQueryLifecycle";
import { myEventsQuery } from "./my-events";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true, $RefreshSig$: () => (type: unknown) => type, $RefreshReg$: () => {} });
});

const identity = (signer: string): AuthIdentity => ({ signer, ephemeral: { address: signer, privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] });
const ALICE = identity("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
const BOB = identity("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

function seeded() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  client.setQueryData(myEventsQuery(ALICE).queryKey, []);
  client.setQueryData(["events", "calendar"], []);
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return { client, wrapper };
}

afterEach(() => cleanup());

describe("when the member's My Events read is cached", () => {
  it("should keep it under the account's private key", () => {
    expect(myEventsQuery(ALICE).queryKey).toEqual(["private", ALICE.signer.toLowerCase(), "events", "mine"]);
  });

  it("should drop it on sign-out", () => {
    const { client, wrapper } = seeded();
    const { rerender } = renderHook(({ account }) => usePrivateQueryLifecycle(account), { wrapper, initialProps: { account: ALICE.signer as string | null } });
    rerender({ account: null });
    expect(client.getQueryData(myEventsQuery(ALICE).queryKey)).toBeUndefined();
    expect(client.getQueryData(["events", "calendar"])).toEqual([]);
  });

  it("should drop the previous account's on an account switch", () => {
    const { client, wrapper } = seeded();
    const { rerender } = renderHook(({ account }) => usePrivateQueryLifecycle(account), { wrapper, initialProps: { account: ALICE.signer } });
    rerender({ account: BOB.signer });
    expect(client.getQueryData(myEventsQuery(ALICE).queryKey)).toBeUndefined();
  });

  it("should refresh it, and leave other private reads alone, after an event write", async () => {
    const { client } = seeded();
    client.setQueryData(["private", ALICE.signer.toLowerCase(), "attendance", "list"], []);
    await invalidateEventsAfterWrite(client);
    expect(client.getQueryState(myEventsQuery(ALICE).queryKey)?.isInvalidated).toBe(true);
    expect(client.getQueryState(["events", "calendar"])?.isInvalidated).toBe(true);
    expect(client.getQueryState(["private", ALICE.signer.toLowerCase(), "attendance", "list"])?.isInvalidated).toBe(false);
  });
});

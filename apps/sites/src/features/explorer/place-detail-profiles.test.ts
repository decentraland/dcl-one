// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { fetchCommunityProfiles } from "@ui/data/hooks/useCommunityProfiles";

import { usePlaceDetailChat, usePlaceDetailCreator } from "./place-detail-profiles";

vi.mock("@ui/data/hooks/useCommunityProfiles", () => ({ fetchCommunityProfiles: vi.fn() }));

const OWNER = "0x797066a17f83425c1b4c7a8cca52d19095520a52";
const lookup = vi.mocked(fetchCommunityProfiles);

afterEach(() => {
  cleanup();
  lookup.mockReset();
});

function queryWrapper() {
  const client = new QueryClient();
  return ({ children }: { children: ReactNode }) => createElement(QueryClientProvider, { client }, children);
}

describe("when crediting the scene creator", () => {
  beforeEach(() => lookup.mockResolvedValue({}));

  it("should use the canonical Foundation credit without looking up a wallet", () => {
    const { result } = renderHook(() => usePlaceDetailCreator({ title: "Scene", contact_name: " decentraland foundation ", owner: OWNER }), { wrapper: queryWrapper() });
    expect(result.current).toMatchObject({ name: "Decentraland Foundation", foundation: true });
    expect(lookup).not.toHaveBeenCalled();
  });

  it("should credit the title when the contact is the SDK default", () => {
    const { result } = renderHook(() => usePlaceDetailCreator({ title: "Scene", contact_name: "sdk", owner: OWNER }), { wrapper: queryWrapper() });
    expect(result.current).toMatchObject({ name: "Scene" });
    expect(result.current?.address).toBeUndefined();
    expect(lookup).not.toHaveBeenCalled();
  });

  it("should keep the declared credit and add the owner's face once it is resolved", async () => {
    lookup.mockResolvedValueOnce({ [OWNER]: { name: "Owner", picture: "/face.png", claimed: true } });
    const { result } = renderHook(() => usePlaceDetailCreator({ title: "Scene", contact_name: "Studio", owner: OWNER }), { wrapper: queryWrapper() });
    expect(result.current?.avatarUrl).toMatch(/^data:image\/svg/);
    await waitFor(() => expect(result.current).toMatchObject({ name: "Studio", address: OWNER, avatarUrl: "/face.png" }));
  });
});

describe("when a wallet appears in scene chat", () => {
  beforeEach(() => lookup.mockResolvedValue({}));

  it("should show its short address after the profile lookup returns no match", async () => {
    const { result } = renderHook(() => usePlaceDetailChat({ status: "ready", messages: [{ sender: OWNER, timestamp: 1, message: "gm" }] }), { wrapper: queryWrapper() });
    expect(result.current.messages[0]?.name).toBe("…");
    await waitFor(() => expect(result.current.messages[0]).toMatchObject({ name: "0x7970...0a52", sender: OWNER, message: "gm" }));
    expect(lookup).toHaveBeenCalledWith([OWNER], expect.objectContaining({ signal: expect.any(AbortSignal) }));
  });
});

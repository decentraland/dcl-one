// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { fetchMemberProfiles } from "@/data/lib/catalyst/social/member-profiles";

import type { SceneChat } from "./scene-chat/useSceneChat";
import { usePlaceDetailChat, usePlaceDetailCreator } from "./place-detail-profiles";

vi.mock("@/data/lib/catalyst/social/member-profiles", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/data/lib/catalyst/social/member-profiles")>()),
  fetchMemberProfiles: vi.fn(),
}));

const OWNER = "0x797066a17f83425c1b4c7a8cca52d19095520a52";
const lookup = vi.mocked(fetchMemberProfiles);

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

function queryWrapper() {
  const client = new QueryClient();
  return ({ children }: { children: ReactNode }) => createElement(QueryClientProvider, { client }, children);
}

describe("when crediting the scene creator", () => {
  let place: Parameters<typeof usePlaceDetailCreator>[0];

  beforeEach(() => {
    lookup.mockResolvedValue({});
  });

  describe("and the contact is the Decentraland Foundation", () => {
    beforeEach(() => {
      place = { title: "Scene", contact_name: " decentraland foundation ", owner: OWNER };
    });

    it("should use the canonical Foundation credit", () => {
      const { result } = renderHook(() => usePlaceDetailCreator(place), { wrapper: queryWrapper() });
      expect(result.current).toMatchObject({ name: "Decentraland Foundation", foundation: true });
    });

    it("should not look up a wallet", () => {
      renderHook(() => usePlaceDetailCreator(place), { wrapper: queryWrapper() });
      expect(lookup).not.toHaveBeenCalled();
    });
  });

  describe("and the contact is the SDK default", () => {
    beforeEach(() => {
      place = { title: "Scene", contact_name: "sdk", owner: OWNER };
    });

    it("should credit the title without an address", () => {
      const { result } = renderHook(() => usePlaceDetailCreator(place), { wrapper: queryWrapper() });
      expect(result.current).toMatchObject({ name: "Scene" });
      expect(result.current?.address).toBeUndefined();
    });

    it("should not look up a wallet", () => {
      renderHook(() => usePlaceDetailCreator(place), { wrapper: queryWrapper() });
      expect(lookup).not.toHaveBeenCalled();
    });
  });

  describe("and the contact is a declared studio name", () => {
    beforeEach(() => {
      place = { title: "Scene", contact_name: "Studio", owner: OWNER };
      lookup.mockResolvedValueOnce({ [OWNER]: { name: "Owner", picture: "/face.png", claimed: true } });
    });

    it("should show a synthetic avatar before the owner's profile resolves", () => {
      const { result } = renderHook(() => usePlaceDetailCreator(place), { wrapper: queryWrapper() });
      expect(result.current?.avatarUrl).toMatch(/^data:image\/svg/);
    });

    it("should keep the declared credit and add the owner's face once it is resolved", async () => {
      const { result } = renderHook(() => usePlaceDetailCreator(place), { wrapper: queryWrapper() });
      await waitFor(() => expect(result.current).toMatchObject({ name: "Studio", address: OWNER, avatarUrl: "/face.png" }));
    });
  });
});

describe("when a wallet appears in scene chat", () => {
  let chat: SceneChat;

  beforeEach(() => {
    lookup.mockResolvedValue({});
    chat = { status: "ready", messages: [{ sender: OWNER, timestamp: 1, message: "gm" }] };
  });

  describe("and the profile lookup returns no match", () => {
    it("should show a placeholder name while the lookup is pending", () => {
      const { result } = renderHook(() => usePlaceDetailChat(chat), { wrapper: queryWrapper() });
      expect(result.current.messages[0]?.name).toBe("…");
    });

    it("should show its short address once the lookup returns", async () => {
      const { result } = renderHook(() => usePlaceDetailChat(chat), { wrapper: queryWrapper() });
      await waitFor(() => expect(result.current.messages[0]).toMatchObject({ name: "0x7970...0a52", sender: OWNER, message: "gm" }));
    });

    it("should look up the sender with an abortable request", async () => {
      const { result } = renderHook(() => usePlaceDetailChat(chat), { wrapper: queryWrapper() });
      await waitFor(() => expect(result.current.messages[0]?.name).toBe("0x7970...0a52"));
      expect(lookup).toHaveBeenCalledWith([OWNER], expect.objectContaining({ signal: expect.any(AbortSignal) }));
    });
  });
});

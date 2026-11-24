// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { fetchProfile } = vi.hoisted(() => ({ fetchProfile: vi.fn() }));
vi.mock("../catalyst/social/profile", () => ({ fetchProfile }));

import { useProfileIdentity } from "./use-profile-name";

const ADDRESS = "0x00000000000000000000000000000000000000aa";
const FACE = "https://profile-images.decentraland.org/entities/bafyface/face.png";
const BODY = "https://profile-images.decentraland.org/entities/bafyface/body.png";

const avatar = (overrides: Record<string, unknown> = {}) => ({
  name: "Ada",
  hasClaimedName: true,
  ethAddress: ADDRESS,
  avatar: { snapshots: { face256: FACE, body: BODY } },
  ...overrides,
});

/** Lets the hook settle the profile read it started. */
const settle = () => act(() => new Promise((resolve) => setTimeout(resolve, 0)));

const EMPTY = { name: "", avatarUrl: "", bodyUrl: "", avatarColor: "" };

afterEach(() => {
  vi.resetAllMocks();
});

describe("useProfileIdentity", () => {
  describe("when the signed-in profile has an avatar", () => {
    it("should return its name, face, body and avatar colour", async () => {
      fetchProfile.mockResolvedValue(avatar());
      const { result } = renderHook(() => useProfileIdentity(ADDRESS, true));
      await waitFor(() => expect(result.current.name).toBe("Ada"));
      expect(result.current.avatarUrl).toMatch(/\/profile-images\/entities\/bafyface\/face\.png$/);
      expect(result.current.bodyUrl).toMatch(/\/profile-images\/entities\/bafyface\/body\.png$/);
      expect(result.current.avatarColor).not.toBe("");
    });
  });

  describe("when the profile has no snapshots", () => {
    it("should return the name without an avatar, so the header keeps the Decentraland mark", async () => {
      fetchProfile.mockResolvedValue(avatar({ avatar: {} }));
      const { result } = renderHook(() => useProfileIdentity(ADDRESS, true));
      await waitFor(() => expect(result.current.name).toBe("Ada"));
      expect(result.current.avatarUrl).toBe("");
      expect(result.current.bodyUrl).toBe("");
    });
  });

  describe("when the account has no profile", () => {
    it("should stay empty", async () => {
      fetchProfile.mockResolvedValue(null);
      const { result } = renderHook(() => useProfileIdentity(ADDRESS, true));
      await waitFor(() => expect(fetchProfile).toHaveBeenCalledWith(ADDRESS));
      await settle();
      expect(result.current).toEqual(EMPTY);
    });
  });

  describe("when the profile read fails", () => {
    it("should stay empty", async () => {
      fetchProfile.mockRejectedValue(new Error("Profile returned 502"));
      const { result } = renderHook(() => useProfileIdentity(ADDRESS, true));
      await waitFor(() => expect(fetchProfile).toHaveBeenCalledWith(ADDRESS));
      await settle();
      expect(result.current).toEqual(EMPTY);
    });
  });

  describe("when the node answers with another account's profile", () => {
    it("should not show that account's name or avatar", async () => {
      fetchProfile.mockResolvedValue(avatar({ ethAddress: "0x00000000000000000000000000000000000000bb" }));
      const { result } = renderHook(() => useProfileIdentity(ADDRESS, true));
      await waitFor(() => expect(fetchProfile).toHaveBeenCalledWith(ADDRESS));
      await settle();
      expect(result.current).toEqual(EMPTY);
    });
  });

  describe("when the visitor is signed out", () => {
    it("should not read a profile", () => {
      const { result } = renderHook(() => useProfileIdentity(ADDRESS, false));
      expect(fetchProfile).not.toHaveBeenCalled();
      expect(result.current).toEqual(EMPTY);
    });
  });

  describe("when the account changes", () => {
    it("should not keep the previous account's identity", async () => {
      fetchProfile.mockResolvedValueOnce(avatar());
      const { result, rerender } = renderHook(({ address }) => useProfileIdentity(address, true), {
        initialProps: { address: ADDRESS },
      });
      await waitFor(() => expect(result.current.name).toBe("Ada"));
      fetchProfile.mockReturnValueOnce(new Promise(() => {}));
      rerender({ address: "0x00000000000000000000000000000000000000cc" });
      expect(result.current).toEqual(EMPTY);
    });
  });
});

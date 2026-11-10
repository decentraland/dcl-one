import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, test, vi } from "vitest";
import { fetchCommunityProfiles, useCommunityProfiles } from "./useCommunityProfiles";

const first = "0x1111111111111111111111111111111111111111";
const second = "0x2222222222222222222222222222222222222222";
const member = (memberAddress: string, name = "", profilePictureUrl = "") => ({ memberAddress, name, profilePictureUrl, hasClaimedName: false, role: "member", joinedAt: null });
afterEach(() => vi.restoreAllMocks());

test("fills member names and faces in a single batch, matches by wallet and preserves existing profiles", async () => {
  const fetch = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify([
    { avatars: [{ ethAddress: second, name: "Second member", avatar: { snapshots: { face256: "second-face-hash" } } }] },
    { avatars: [{ userId: first, name: "First member", hasClaimedName: true, avatar: { snapshots: { face256: "https://images.test/first.png" } } }] },
  ])));
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const members = [member(first, first), member(second), member("0x3333333333333333333333333333333333333333", "Already known", "https://images.test/known.png")];
  const { result } = renderHook(() => useCommunityProfiles(members, true), { wrapper: ({ children }) => <QueryClientProvider client={client}>{children}</QueryClientProvider> });
  await waitFor(() => expect(result.current[0]?.name).toBe("First member"));
  expect(result.current[0]).toMatchObject({ profilePictureUrl: "https://images.test/first.png", hasClaimedName: true });
  expect(result.current[1]?.name).toBe("Second member");
  expect(result.current[1]?.profilePictureUrl).toContain("/content/contents/second-face-hash");
  expect(result.current[2]?.name).toBe("Already known");
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(JSON.parse(String(fetch.mock.calls[0]?.[1]?.body))).toEqual({ ids: [first, second] });
});

test("preserves member rows when profiles are unavailable", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("unavailable", { status: 503 }));
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const { result } = renderHook(() => useCommunityProfiles([member(first)], true), { wrapper: ({ children }) => <QueryClientProvider client={client}>{children}</QueryClientProvider> });
  await waitFor(() => expect(client.isFetching()).toBe(0));
  expect(result.current[0]).toMatchObject({ memberAddress: first, name: "", profilePictureUrl: "" });
});

test("keeps a name whose profile leaves its passport fields null, as the Catalyst serves an unfilled one", async () => {
  const unfilled = { country: null, gender: null, pronouns: null, relationshipStatus: null, sexualOrientation: null, language: null, profession: null, realName: null, hobbies: null };
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify([{ avatars: [{ ethAddress: first, name: "MetaWolf", hasClaimedName: false, ...unfilled }] }])));
  const profiles = await fetchCommunityProfiles([first]);
  expect(profiles[first]).toMatchObject({ name: "MetaWolf", claimed: false });
});

test("reads every profile of a batch even when one carries a null birthdate or an unreadable shape", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify([
    { avatars: [{ ethAddress: first, name: "First member", birthdate: null }] },
    { avatars: [{ ethAddress: "0x3333333333333333333333333333333333333333", name: 7 }] },
    { avatars: [{ ethAddress: second, name: "Second member" }] },
  ])));
  const profiles = await fetchCommunityProfiles([first, second, "0x3333333333333333333333333333333333333333"]);
  expect(profiles[first]?.name).toBe("First member");
  expect(profiles[second]?.name).toBe("Second member");
  expect(profiles["0x3333333333333333333333333333333333333333"]).toBeUndefined();
});

describe.each([undefined, "https://profiles.example"])(
  "when reading profile faces with base %s",
  (base) => {
    let fetchImpl: typeof fetch;
    beforeEach(() => {
      fetchImpl = vi.fn(
        async () =>
          new Response(
            JSON.stringify([
              {
                avatars: [
                  {
                    ethAddress: first,
                    name: "Profile",
                    avatar: { snapshots: { face256: "profilehash" } },
                  },
                ],
              },
            ]),
          ),
      ) as typeof fetch;
    });
    it("should use the same node for the lookup and the resulting content face", async () => {
      const profiles = await fetchCommunityProfiles([first], {
        ...(base ? { base } : {}),
        fetchImpl,
      });
      const requested = String(vi.mocked(fetchImpl).mock.calls[0]?.[0]);
      expect(profiles[first]?.picture).toBe(
        requested.replace("/lambdas/profiles", "/content/contents/profilehash"),
      );
    });
  },
);

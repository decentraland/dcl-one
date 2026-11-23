import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const net = vi.hoisted(() => ({
  fetchProfile: vi.fn(),
  fetchProfileCreations: vi.fn(),
  fetchProfileAssets: vi.fn(),
  fetchProfilePlaces: vi.fn(),
  fetchUserPhotos: vi.fn(),
  fetchUserBadges: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/social/badges", () => ({ fetchUserBadges: net.fetchUserBadges }));
vi.mock("@/data/lib/catalyst/social/profile", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/profile")>()),
  fetchProfile: net.fetchProfile,
}));
vi.mock("@/data/lib/catalyst/marketplace/profile-collectibles", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/marketplace/profile-collectibles")>()),
  fetchProfileCreations: net.fetchProfileCreations,
  fetchProfileAssets: net.fetchProfileAssets,
}));
vi.mock("@/data/lib/catalyst/places/profile-places", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/profile-places")>()),
  fetchProfilePlaces: net.fetchProfilePlaces,
}));
vi.mock("@/data/lib/catalyst/social/photos", () => ({ fetchUserPhotos: net.fetchUserPhotos }));
const flags = vi.hoisted(() => ({ dappsFlags: vi.fn() }));
vi.mock("@/data/lib/feature-flags.server", () => ({ knownDappsFlags: flags.dappsFlags }));

import { createStaticHandler, RouterContextProvider } from "react-router";

import { markPublicResponse, requestSessionContext } from "@/core/lib/telemetry/session";
import { CatalystError } from "@/data/lib/catalyst/client";
import { profileKeys } from "@/data/lib/profile/queries";

import {
  legacyAccountsRedirect,
  OPTIONAL_READS_BUDGET_MS,
  ownProfileRedirect,
  profileLoader,
  type ProfileLoaderData,
} from "./profile-loader.server";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const B = "0x1111111111111111111111111111111111111111";
const avatar = (name: string) => ({
  name,
  hasClaimedName: true,
  description: "",
  avatar: { snapshots: { face256: "" } },
});

/** `wallet` sets the wallet cookie every sign-in method sets (`dcl_wallet`), as a browser wallet leaves it. */
function request(path: string, wallet?: string, init: RequestInit = {}) {
  return new Request(`http://localhost${path}`, {
    ...init,
    headers: wallet ? { cookie: `dcl_wallet=${wallet}` } : {},
  });
}

/** Runs the real loader, returning its loader data, or the response it threw. */
async function run(path: string, params: { address?: string; tab?: string }, wallet?: string) {
  try {
    const result = (await profileLoader(request(path, wallet), new RouterContextProvider(), params)) as unknown as { data: ProfileLoaderData };
    return { ok: true as const, data: result.data };
  } catch (thrown) {
    return { ok: false as const, thrown };
  }
}

const statusOf = (thrown: unknown) =>
  thrown instanceof Response ? thrown.status : (thrown as { init?: { status?: number } }).init?.status;
const locationOf = (thrown: unknown) => (thrown as Response).headers.get("Location");
function thrownBy(fn: () => unknown): Response {
  try {
    fn();
  } catch (thrown) {
    expect(thrown).toBeInstanceOf(Response);
    return thrown as Response;
  }
  throw new Error("expected a redirect to be thrown");
}
const keysOf = (data: ProfileLoaderData) => data.dehydratedState.queries.map((q) => q.queryKey);

type Outcome = Awaited<ReturnType<typeof run>>;
const EMPTY_PAGE = { items: [], received: 0, total: 0 };

beforeEach(() => {
  net.fetchProfile.mockImplementation(async (address: string) => avatar(address === A ? "Alice" : "Bob"));
  net.fetchProfileCreations.mockResolvedValue(EMPTY_PAGE);
  flags.dappsFlags.mockResolvedValue({ "dapps-show-referral": true });
});
afterEach(() => vi.resetAllMocks());

describe("profile loader, required data", () => {
  let out: Outcome;

  describe("when the address cannot be a wallet", () => {
    beforeEach(async () => {
      out = await run("/profile/not-a-wallet", { address: "not-a-wallet" });
    });

    it("should answer 404", () => {
      expect(out.ok).toBe(false);
      expect(statusOf(out.ok ? null : out.thrown)).toBe(404);
    });

    it("should not read anything", () => {
      expect(net.fetchProfile).not.toHaveBeenCalled();
    });
  });

  describe("when the profile service fails", () => {
    beforeEach(async () => {
      net.fetchProfile.mockRejectedValue(new CatalystError("Catalyst returned 502", "/lambdas/profile", 502));

      out = await run(`/profile/${A}`, { address: A });
    });

    it("should answer 503", () => {
      expect(out.ok).toBe(false);
      expect(statusOf(out.ok ? null : out.thrown)).toBe(503);
    });
  });

  describe("when the profile read has a bug", () => {
    beforeEach(async () => {
      net.fetchProfile.mockRejectedValue(new TypeError("Cannot read properties of undefined (reading 'name')"));

      out = await run(`/profile/${A}`, { address: A });
    });

    it("should let it become a 500, not a 503", () => {
      expect(out.ok).toBe(false);
      expect(out.ok ? null : out.thrown).toBeInstanceOf(TypeError);
    });
  });

  describe("when the wallet has no profile yet", () => {
    beforeEach(async () => {
      net.fetchProfile.mockResolvedValue(null);

      out = await run(`/profile/${A}`, { address: A });
    });

    it("should render rather than fail", () => {
      expect(out.ok).toBe(true);
    });

    it("should render an empty profile", () => {
      const entry = out.ok ? out.data.dehydratedState.queries[0] : undefined;
      expect(entry?.queryKey).toEqual(profileKeys.profile(A));
      expect(entry?.state.data).toBeNull();
    });

    it("should leave the link-preview card generic", () => {
      expect(out.ok && out.data.card).toEqual({});
    });
  });

  describe("when the profile has a name and a description", () => {
    beforeEach(async () => {
      net.fetchProfile.mockResolvedValue({ ...avatar("Alice"), description: "Builder.\nHost." });

      out = await run(`/profile/${A}/photos`, { address: A, tab: "photos" });
    });

    it("should give the link-preview card its name and one-line description", () => {
      expect(out.ok && out.data.card).toEqual({ name: "Alice", description: "Builder. Host." });
    });
  });
});

describe("profile loader, dehydrated state", () => {
  let out: Outcome;

  describe("when the Creations tab opens", () => {
    beforeEach(async () => {
      out = await run(`/profile/${A}/creations`, { address: A, tab: "creations" });
    });

    // Creations opens on wearables: only that list's first page is read.
    it("should dehydrate the profile and the active tab's data only", () => {
      expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A), profileKeys.creations(A, "wearable")]);
    });

    it("should read only the first page of wearables", () => {
      expect(net.fetchProfileCreations).toHaveBeenCalledTimes(1);
      expect(net.fetchProfileCreations.mock.calls[0]?.[1]).toMatchObject({ category: "wearable", skip: 0 });
    });

    it("should not read other tabs' data", () => {
      expect(net.fetchProfilePlaces).not.toHaveBeenCalled();
      expect(net.fetchUserPhotos).not.toHaveBeenCalled();
    });
  });

  describe("when a wallet with no profile has badges", () => {
    beforeEach(async () => {
      net.fetchProfile.mockResolvedValue(null);
      net.fetchUserBadges.mockResolvedValue([{ id: "x", name: "X" }]);

      out = await run(`/profile/${A}`, { address: A });
    });

    it("should not hand it its badges", () => {
      expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A)]);
    });
  });

  describe("when an optional read outlasts the budget", () => {
    let settled: boolean;
    let result: ReturnType<typeof run>;
    beforeEach(async () => {
      vi.useFakeTimers();
      settled = false;
      net.fetchProfileCreations.mockReturnValue(new Promise(() => {}));
      result = run(`/profile/${A}/creations`, { address: A, tab: "creations" });
      void result.then(() => { settled = true; });
      await vi.advanceTimersByTimeAsync(OPTIONAL_READS_BUDGET_MS - 1);
    });
    afterEach(() => vi.useRealTimers());
    it("should still wait before the deadline", () => {
      expect(settled).toBe(false);
    });
    describe("and the deadline passes", () => {
      beforeEach(async () => {
        await vi.advanceTimersByTimeAsync(1);
        out = await result;
      });
      it("should render only the profile", () => {
        expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A)]);
      });
    });
  });

  describe("when a wallet has no profile and badges never finish", () => {
    let settled: boolean;
    let result: Promise<void>;
    beforeEach(async () => {
      vi.useFakeTimers();
      settled = false;
      net.fetchProfile.mockResolvedValue(null);
      net.fetchUserBadges.mockReturnValue(new Promise(() => {}));
      result = run(`/profile/${A}`, { address: A }).then(() => { settled = true; });
      await vi.advanceTimersByTimeAsync(0);
    });
    afterEach(async () => {
      await result;
      vi.useRealTimers();
    });
    it("should return without consuming the optional budget", () => {
      expect(settled).toBe(true);
    });
  });

  describe("when Overview opens", () => {
    beforeEach(async () => {
      net.fetchUserBadges.mockResolvedValue([]);

      out = await run(`/profile/${A}`, { address: A });
    });

    it("should dehydrate the badges with the profile", () => {
      expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A), profileKeys.badges(A)]);
    });
  });

  describe("when an optional read fails", () => {
    beforeEach(async () => {
      net.fetchProfileCreations.mockRejectedValue(new Error("marketplace down"));

      out = await run(`/profile/${A}/creations`, { address: A, tab: "creations" });
    });

    it("should leave it to the browser and still render", () => {
      expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A)]);
    });
  });

  describe("when another member opens a profile's Assets", () => {
    beforeEach(async () => {
      out = await run(`/profile/${A}/assets`, { address: A, tab: "assets" }, B);
    });

    it("should redirect to the profile root", () => {
      expect(out.ok).toBe(false);
      expect(locationOf(out.ok ? null : out.thrown)).toBe(`/profile/${A}`);
    });

    it("should not read the assets", () => {
      expect(net.fetchProfileAssets).not.toHaveBeenCalled();
    });
  });

  describe("when the signed-in member opens their own Assets", () => {
    beforeEach(async () => {
      net.fetchProfileAssets.mockResolvedValue(EMPTY_PAGE);

      out = await run(`/profile/${A}/assets`, { address: A, tab: "assets" }, A);
    });

    it("should mark the profile as their own", () => {
      expect(out.ok && out.data.own).toBe(true);
    });

    it("should dehydrate the flags, the profile and the assets", () => {
      expect(out.ok && keysOf(out.data)).toEqual([profileKeys.flags(), profileKeys.profile(A), profileKeys.assets(A, null)]);
    });

    it("should read every category's first page", () => {
      expect(net.fetchProfileAssets.mock.calls[0]?.[1]).toMatchObject({ category: undefined, skip: 0 });
    });
  });

  describe("when the owner opens Photos", () => {
    beforeEach(async () => {
      net.fetchUserPhotos.mockResolvedValue(EMPTY_PAGE);

      out = await run(`/profile/${A}/photos`, { address: A, tab: "photos" }, A);
    });

    it("should dehydrate the flags, the profile and the photos", () => {
      expect(out.ok && keysOf(out.data)).toEqual([profileKeys.flags(), profileKeys.profile(A), profileKeys.photos(A)]);
    });

    it("should read the public gallery's first page once", () => {
      expect(net.fetchUserPhotos).toHaveBeenCalledTimes(1);
      expect(net.fetchUserPhotos.mock.calls[0]?.[1]).toEqual({ limit: 24, offset: 0 });
    });

    it("should read it unsigned", () => {
      expect(net.fetchUserPhotos.mock.calls[0]?.[2]).not.toHaveProperty("identity");
    });
  });

  describe("when two requests for different profiles interleave", () => {
    let firstOut: Outcome;
    let second: Outcome;
    const nameOf = (result: Outcome) =>
      result.ok ? (result.data.dehydratedState.queries[0]?.state.data as { name: string }).name : null;

    beforeEach(async () => {
      let releaseA!: () => void;
      net.fetchProfile.mockImplementation((address: string) =>
        address === A
          ? new Promise((resolve) => {
              releaseA = () => resolve(avatar("Alice"));
            })
          : Promise.resolve(avatar("Bob")),
      );

      const first = run(`/profile/${A}`, { address: A });
      second = await run(`/profile/${B}`, { address: B });
      releaseA();
      firstOut = await first;
    });

    it("should hand each request its own profile", () => {
      expect(nameOf(firstOut)).toBe("Alice");
      expect(nameOf(second)).toBe("Bob");
    });

    it("should hand each request its own address", () => {
      expect(firstOut.ok && firstOut.data.address).toBe(A);
      expect(second.ok && second.data.address).toBe(B);
    });
  });
});

describe("profile loader, canonical paths", () => {
  describe.each([
    [`/profile/${A}?tab=places`, { address: A }, `/profile/${A}/places`],
    [`/profile/${A}?tab=assets&own=1`, { address: A }, `/profile/${A}`],
    [`/profile/${A}?own=1`, { address: A }, `/profile/${A}`],
    [`/profile/${A.toUpperCase().replace("0X", "0x")}/photos`, { address: A.toUpperCase().replace("0X", "0x"), tab: "photos" }, `/profile/${A}/photos`],
    [`/profile/${A}/unknown`, { address: A, tab: "unknown" }, `/profile/${A}`],
    [`/profile/${A}/Places`, { address: A, tab: "Places" }, `/profile/${A}/places`],
    [`/profile/${A}?tab=places&utm_source=mail`, { address: A }, `/profile/${A}/places?utm_source=mail`],
    [`/profile/${A}/unknown?utm_source=mail&own=1`, { address: A, tab: "unknown" }, `/profile/${A}?utm_source=mail`],
  ])("when the path is %s", (path, params, location) => {
    let out: Outcome;

    beforeEach(async () => {
      out = await run(path, params);
    });

    it(`should redirect to ${location}`, () => {
      expect(out.ok).toBe(false);
      expect(locationOf(out.ok ? null : out.thrown)).toBe(location);
    });
  });

  describe("when /profile/me is opened", () => {
    describe("and the member is signed in", () => {
      it("should send them to their profile and tab", () => {
        expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me/photos", A), "photos")))).toBe(
          `/profile/${A}/photos`,
        );
      });
    });

    describe("and the visitor is signed out", () => {
      it("should send them to sign-in, coming back to the same path", () => {
        expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me/places"), "places")))).toBe(
          `/auth/login?redirectTo=${encodeURIComponent("/profile/me/places")}`,
        );
      });
    });

    describe("and the tab is one the member cannot have", () => {
      it("should send them to the profile root", () => {
        expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me/creations", A), "creations")))).toBe(
          `/profile/${A}`,
        );
      });
    });

    describe("and the link is an old ?tab= link", () => {
      it("should keep the tab", () => {
        expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me?tab=places&utm_source=mail", A))))).toBe(
          `/profile/${A}/places?utm_source=mail`,
        );
      });
    });

    describe("and the member signed in with a browser wallet (no verified cookie)", () => {
      it("should send them to their profile, not to sign-in", () => {
        expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me", A))))).toBe(`/profile/${A}`);
      });
    });

    describe("and only the thirdweb verified cookie is set", () => {
      let verifiedOnly: Request;

      beforeEach(() => {
        verifiedOnly = new Request("http://localhost/profile/me", { headers: { cookie: `dcl_wallet_verified=${A}` } });
      });

      it("should not take it as a sign-in", () => {
        expect(locationOf(thrownBy(() => ownProfileRedirect(verifiedOnly)))).toBe(
          `/auth/login?redirectTo=${encodeURIComponent("/profile/me")}`,
        );
      });
    });
  });

  describe("when an old /profile/accounts path is opened", () => {
    it("should send it to the profile, keeping the query", () => {
      expect(
        locationOf(
          thrownBy(() =>
            legacyAccountsRedirect(request("/profile/accounts/x/Places?utm_source=mail"), {
              address: A.toUpperCase().replace("0X", "0x"),
              tab: "Places",
            }),
          ),
        ),
      ).toBe(`/profile/${A}/places?utm_source=mail`);
    });
  });

  describe("when a browser-wallet member deep links to an own-only tab", () => {
    let out: Outcome;

    beforeEach(async () => {
      net.fetchProfileAssets.mockResolvedValue(EMPTY_PAGE);

      out = await run(`/profile/${A}/assets`, { address: A, tab: "assets" }, A);
    });

    it("should open it", () => {
      expect(out.ok && out.data.own).toBe(true);
    });
  });
});

describe("profile loader, a visitor who leaves", () => {
  let seen: AbortSignal | undefined;
  let controller: AbortController;

  beforeEach(() => {
    seen = undefined;
    controller = new AbortController();
  });

  describe("when the request is already aborted when the loader starts", () => {
    beforeEach(() => {
      net.fetchProfile.mockImplementation((_address: string, opts: { signal?: AbortSignal }) => {
        seen = opts.signal;
        return new Promise((_resolve, reject) => {
          if (opts.signal?.aborted) reject(new DOMException("aborted", "AbortError"));
          opts.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
        });
      });
      controller.abort();
    });

    it("should reject without reading a profile", async () => {
      await expect(
        profileLoader(request(`/profile/${A}`, undefined, { signal: controller.signal }), new RouterContextProvider(), { address: A }),
      ).rejects.toBeTruthy();
      expect(seen).toBeUndefined();
      expect(net.fetchProfile).not.toHaveBeenCalled();
    });
  });

  describe("when the request is aborted during the profile read", () => {
    let pending: Promise<unknown>;

    beforeEach(async () => {
      net.fetchProfile.mockImplementation(
        (_address: string, opts: { signal?: AbortSignal }) =>
          new Promise((_resolve, reject) => {
            seen = opts.signal;
            opts.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
          }),
      );
      pending = profileLoader(request(`/profile/${A}`, undefined, { signal: controller.signal }), new RouterContextProvider(), { address: A });
      await vi.waitFor(() => expect(seen).toBeDefined());
    });

    it("should cancel the profile read", async () => {
      controller.abort();

      await expect(pending).rejects.toBeTruthy();
      expect(seen?.aborted).toBe(true);
    });
  });
});

describe("profile loader session", () => {
  type Wrapped = { data: ProfileLoaderData; init?: { headers?: HeadersInit } };
  const setCookies = (result: Wrapped) => new Headers(result.init?.headers).getSetCookie();
  let context: RouterContextProvider;
  let result: Wrapped;

  beforeEach(() => {
    context = new RouterContextProvider();
  });

  describe("when the root middleware set the session", () => {
    beforeEach(async () => {
      context.set(requestSessionContext, { sid: "from-middleware", created: true });

      result = (await profileLoader(request(`/profile/${A}`), context, { address: A })) as unknown as Wrapped;
    });

    it("should use that session", () => {
      expect(result.data.sid).toBe("from-middleware");
    });

    it("should leave the cookie to the middleware", () => {
      expect(setCookies(result)).toEqual([]);
    });
  });

  describe("when no middleware ran", () => {
    beforeEach(async () => {
      result = (await profileLoader(request(`/profile/${A}`), context, { address: A })) as unknown as Wrapped;
    });

    it("should mint a session and set its one cookie", () => {
      expect(setCookies(result)).toHaveLength(1);
      expect(setCookies(result)[0]).toContain(`sid=${encodeURIComponent(result.data.sid)}`);
    });
  });

  describe("when the context is marked public", () => {
    beforeEach(() => {
      markPublicResponse(context);
    });

    it("should refuse it, so no sid reaches shared page data, without reading the profile", async () => {
      await expect(profileLoader(request(`/profile/${A}`), context, { address: A })).rejects.toThrow(/publicly cacheable/);
      expect(net.fetchProfile).not.toHaveBeenCalled();
    });
  });
});

describe("profile document request, through the router's handler", () => {
  let handler: ReturnType<typeof createStaticHandler>;

  beforeEach(() => {
    handler = createStaticHandler([
      {
        id: "profile",
        path: "/profile/:address/:tab?",
        loader: ({ request, context, params }) => profileLoader(request, context, params),
        ErrorBoundary: () => null,
      },
    ]);
  });

  describe.each([
    ["an address that cannot be a wallet", "/profile/not-a-wallet", () => undefined, 404],
    [
      "a failing profile service",
      `/profile/${A}`,
      () => net.fetchProfile.mockRejectedValue(new CatalystError("Catalyst unreachable", "/lambdas/profile", 0)),
      503,
    ],
  ])("when the request meets %s", (_case, path, arrange, status) => {
    let context: unknown;

    beforeEach(async () => {
      arrange();

      context = await handler.query(new Request(`http://localhost${path}`));
    });

    it(`should answer ${status} with the loader's status`, () => {
      expect(context).not.toBeInstanceOf(Response);
      expect((context as { statusCode: number }).statusCode).toBe(status);
    });
  });

  describe("when the request is for a wallet", () => {
    let context: { statusCode: number; loaderData: Record<string, ProfileLoaderData> };

    beforeEach(async () => {
      context = (await handler.query(new Request(`http://localhost/profile/${A}`))) as typeof context;
    });

    it("should answer 200", () => {
      expect(context.statusCode).toBe(200);
    });

    it("should hand over the dehydrated profile", () => {
      expect(context.loaderData.profile?.dehydratedState.queries[0]?.queryKey).toEqual(profileKeys.profile(A));
    });
  });
});

describe("profile loader, Referral Rewards", () => {
  const referral = `/profile/${A}/referral-rewards`;
  let out: Outcome;

  describe("when the member opens their own Referral Rewards", () => {
    describe("and the flag is on", () => {
      beforeEach(async () => {
        out = await run(referral, { address: A, tab: "referral-rewards" }, A);
      });

      it("should open it", () => {
        expect(out.ok).toBe(true);
      });

      it("should hand the page the flag", () => {
        const entry = out.ok ? out.data.dehydratedState.queries.find((q) => q.queryHash === JSON.stringify(profileKeys.flags())) : undefined;
        expect(entry?.state.data).toEqual({ referral: true });
      });
    });

    describe("and the flag is off", () => {
      beforeEach(async () => {
        flags.dappsFlags.mockResolvedValue({});

        out = await run(referral, { address: A, tab: "referral-rewards" }, A);
      });

      it("should send the tab to the profile root", () => {
        expect(out.ok).toBe(false);
        expect(locationOf(out.ok ? null : out.thrown)).toBe(`/profile/${A}`);
      });
    });

    describe("and the flags are unavailable", () => {
      beforeEach(async () => {
        flags.dappsFlags.mockResolvedValue(null);

        out = await run(referral, { address: A, tab: "referral-rewards" }, A);
      });

      it("should keep the referral URL", () => {
        expect(out.ok).toBe(true);
      });

      it("should leave the flags query unseeded", () => {
        expect(out.ok && out.data.dehydratedState.queries.some((query) => query.queryKey.includes("flags"))).toBe(false);
      });
    });
  });

  describe("when the profile site's old `referral` tab id is opened", () => {
    beforeEach(async () => {
      out = await run(`/profile/${A}/referral?utm_source=mail`, { address: A, tab: "referral" }, A);
    });

    it("should send it to Referral Rewards, keeping the query", () => {
      expect(out.ok).toBe(false);
      expect(locationOf(out.ok ? null : out.thrown)).toBe(`${referral}?utm_source=mail`);
    });
  });

  describe("when the old `?tab=referral` link is opened", () => {
    beforeEach(async () => {
      out = await run(`/profile/${A}?tab=referral`, { address: A }, A);
    });

    it("should send it to Referral Rewards", () => {
      expect(locationOf(out.ok ? null : out.thrown)).toBe(referral);
    });
  });

  describe("when another member's Referral Rewards is opened", () => {
    beforeEach(async () => {
      out = await run(`/profile/${B}/referral-rewards`, { address: B, tab: "referral-rewards" }, A);
    });

    it("should not show it, sending the visitor to that profile's root", () => {
      expect(locationOf(out.ok ? null : out.thrown)).toBe(`/profile/${B}`);
    });

    it("should read no flags for them", () => {
      expect(flags.dappsFlags).not.toHaveBeenCalled();
    });
  });

  describe("when /profile/me/referral is opened", () => {
    let thrown: Response;

    beforeEach(() => {
      thrown = thrownBy(() => ownProfileRedirect(request("/profile/me/referral", A), "referral"));
    });

    it("should keep the tab for the profile's own loader to decide", () => {
      expect(locationOf(thrown)).toBe(referral);
    });
  });
});

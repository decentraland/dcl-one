import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const net = vi.hoisted(() => ({
  fetchProfile: vi.fn(),
  fetchCreations: vi.fn(),
  fetchProfilePlaces: vi.fn(),
  fetchUserPhotos: vi.fn(),
  fetchOwnedWearables: vi.fn(),
  fetchOwnedNames: vi.fn(),
  fetchUserBadges: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/social/badges", () => ({ fetchUserBadges: net.fetchUserBadges }));
vi.mock("@/data/lib/catalyst/social/profile", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/profile")>()),
  fetchProfile: net.fetchProfile,
}));
vi.mock("@/data/lib/catalyst/marketplace/index", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/marketplace/index")>()),
  fetchCreations: net.fetchCreations,
}));
vi.mock("@/data/lib/catalyst/places/index", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/index")>()),
  fetchProfilePlaces: net.fetchProfilePlaces,
}));
vi.mock("@/data/lib/catalyst/social/photos", () => ({ fetchUserPhotos: net.fetchUserPhotos }));
vi.mock("@/data/lib/catalyst/marketplace/account", () => ({
  fetchOwnedWearables: net.fetchOwnedWearables,
  fetchOwnedNames: net.fetchOwnedNames,
}));

import { createStaticHandler } from "react-router";

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
    const result = (await profileLoader(request(path, wallet), params)) as unknown as { data: ProfileLoaderData };
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

beforeEach(() => {
  net.fetchProfile.mockImplementation(async (address: string) => avatar(address === A ? "Alice" : "Bob"));
  net.fetchCreations.mockResolvedValue({ wearables: [], emotes: [] });
});
afterEach(() => vi.resetAllMocks());

describe("profile loader, required data", () => {
  it("answers 404 for an address that cannot be a wallet, without reading anything", async () => {
    const out = await run("/profile/not-a-wallet", { address: "not-a-wallet" });

    expect(out.ok).toBe(false);
    expect(statusOf(out.ok ? null : out.thrown)).toBe(404);
    expect(net.fetchProfile).not.toHaveBeenCalled();
  });

  it("answers 503 when the profile service fails", async () => {
    net.fetchProfile.mockRejectedValue(new CatalystError("Catalyst returned 502", "/lambdas/profile", 502));

    const out = await run(`/profile/${A}`, { address: A });

    expect(out.ok).toBe(false);
    expect(statusOf(out.ok ? null : out.thrown)).toBe(503);
  });

  it("lets a bug in the profile read become a 500, not a 503", async () => {
    net.fetchProfile.mockRejectedValue(new TypeError("Cannot read properties of undefined (reading 'name')"));

    const out = await run(`/profile/${A}`, { address: A });

    expect(out.ok).toBe(false);
    expect(out.ok ? null : out.thrown).toBeInstanceOf(TypeError);
  });

  it("renders a wallet with no profile yet as an empty profile, not an error", async () => {
    net.fetchProfile.mockResolvedValue(null);

    const out = await run(`/profile/${A}`, { address: A });

    expect(out.ok).toBe(true);
    const entry = out.ok ? out.data.dehydratedState.queries[0] : undefined;
    expect(entry?.queryKey).toEqual(profileKeys.profile(A));
    expect(entry?.state.data).toBeNull();
  });
});

describe("profile loader, dehydrated state", () => {
  it("dehydrates the profile and the active tab's data only", async () => {
    const out = await run(`/profile/${A}/creations`, { address: A, tab: "creations" });

    expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A), profileKeys.creations(A)]);
    expect(net.fetchProfilePlaces).not.toHaveBeenCalled();
    expect(net.fetchUserPhotos).not.toHaveBeenCalled();
  });

  it("does not hand a wallet with no profile its badges", async () => {
    net.fetchProfile.mockResolvedValue(null);
    net.fetchUserBadges.mockResolvedValue([{ id: "x", name: "X" }]);

    const out = await run(`/profile/${A}`, { address: A });

    expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A)]);
  });

  it("renders without an optional read that outlasts the budget, leaving it to the browser", async () => {
    net.fetchCreations.mockReturnValue(new Promise(() => {}));
    const started = Date.now();

    const out = await run(`/profile/${A}/creations`, { address: A, tab: "creations" });

    expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A)]);
    expect(Date.now() - started).toBeLessThan(OPTIONAL_READS_BUDGET_MS + 1000);
  }, 10_000);

  it("dehydrates the badges with the profile on Overview", async () => {
    net.fetchUserBadges.mockResolvedValue([]);

    const out = await run(`/profile/${A}`, { address: A });

    expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A), profileKeys.badges(A)]);
  });

  it("leaves a failed optional read to the browser and still renders", async () => {
    net.fetchCreations.mockRejectedValue(new Error("marketplace down"));

    const out = await run(`/profile/${A}/creations`, { address: A, tab: "creations" });

    expect(out.ok && keysOf(out.data)).toEqual([profileKeys.profile(A)]);
  });

  it("does not read another member's assets", async () => {
    const out = await run(`/profile/${A}/assets`, { address: A, tab: "assets" }, B);

    expect(out.ok).toBe(false);
    expect(locationOf(out.ok ? null : out.thrown)).toBe(`/profile/${A}`);
    expect(net.fetchOwnedWearables).not.toHaveBeenCalled();
  });

  it("reads the signed-in member's own assets", async () => {
    net.fetchOwnedWearables.mockResolvedValue({ elements: [], total: 0 });
    net.fetchOwnedNames.mockResolvedValue({ elements: [], total: 0 });

    const out = await run(`/profile/${A}/assets`, { address: A, tab: "assets" }, A);

    expect(out.ok && out.data.own).toBe(true);
    expect(out.ok && keysOf(out.data)).toEqual([
      profileKeys.profile(A),
      profileKeys.ownedWearables(A),
      profileKeys.ownedNames(A),
    ]);
  });

  it("keeps two interleaved requests for different profiles apart", async () => {
    let releaseA!: () => void;
    net.fetchProfile.mockImplementation((address: string) =>
      address === A
        ? new Promise((resolve) => {
            releaseA = () => resolve(avatar("Alice"));
          })
        : Promise.resolve(avatar("Bob")),
    );

    const first = run(`/profile/${A}`, { address: A });
    const second = await run(`/profile/${B}`, { address: B });
    releaseA();
    const firstOut = await first;

    const nameOf = (out: Awaited<typeof first>) =>
      out.ok ? (out.data.dehydratedState.queries[0]?.state.data as { name: string }).name : null;
    expect(nameOf(firstOut)).toBe("Alice");
    expect(nameOf(second)).toBe("Bob");
    expect(firstOut.ok && firstOut.data.address).toBe(A);
    expect(second.ok && second.data.address).toBe(B);
  });
});

describe("profile loader, canonical paths", () => {
  it.each([
    [`/profile/${A}?tab=places`, { address: A }, `/profile/${A}/places`],
    [`/profile/${A}?tab=assets&own=1`, { address: A }, `/profile/${A}`],
    [`/profile/${A}?own=1`, { address: A }, `/profile/${A}`],
    [`/profile/${A.toUpperCase().replace("0X", "0x")}/photos`, { address: A.toUpperCase().replace("0X", "0x"), tab: "photos" }, `/profile/${A}/photos`],
    [`/profile/${A}/unknown`, { address: A, tab: "unknown" }, `/profile/${A}`],
    [`/profile/${A}/Places`, { address: A, tab: "Places" }, `/profile/${A}/places`],
    [`/profile/${A}?tab=places&utm_source=mail`, { address: A }, `/profile/${A}/places?utm_source=mail`],
    [`/profile/${A}/unknown?utm_source=mail&own=1`, { address: A, tab: "unknown" }, `/profile/${A}?utm_source=mail`],
  ])("redirects %s", async (path, params, location) => {
    const out = await run(path, params);

    expect(out.ok).toBe(false);
    expect(locationOf(out.ok ? null : out.thrown)).toBe(location);
  });

  it("sends /profile/me to the signed-in member's profile and tab", () => {
    expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me/photos", A), "photos")))).toBe(
      `/profile/${A}/photos`,
    );
  });

  it("sends a signed-out /profile to sign-in, coming back to the same path", () => {
    expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me/places"), "places")))).toBe(
      `/auth/login?redirectTo=${encodeURIComponent("/profile/me/places")}`,
    );
  });

  it("sends an own-profile tab the member cannot have to the profile root", () => {
    expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me/creations", A), "creations")))).toBe(
      `/profile/${A}`,
    );
  });

  it("sends the old /profile/accounts paths to the profile, keeping the query", () => {
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

  it("keeps the tab of the old /profile/me?tab= links", () => {
    expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me?tab=places&utm_source=mail", A))))).toBe(
      `/profile/${A}/places?utm_source=mail`,
    );
  });

  it("sends a browser-wallet member (no verified cookie) on /profile/me to their profile, not to sign-in", () => {
    expect(locationOf(thrownBy(() => ownProfileRedirect(request("/profile/me", A))))).toBe(`/profile/${A}`);
  });

  it("does not take the thirdweb-only verified cookie as a sign-in", () => {
    const verifiedOnly = new Request("http://localhost/profile/me", { headers: { cookie: `dcl_wallet_verified=${A}` } });

    expect(locationOf(thrownBy(() => ownProfileRedirect(verifiedOnly)))).toBe(
      `/auth/login?redirectTo=${encodeURIComponent("/profile/me")}`,
    );
  });

  it("opens a browser-wallet member's own-only tab from a deep link", async () => {
    net.fetchOwnedWearables.mockResolvedValue({ elements: [], total: 0 });
    net.fetchOwnedNames.mockResolvedValue({ elements: [], total: 0 });

    const out = await run(`/profile/${A}/assets`, { address: A, tab: "assets" }, A);

    expect(out.ok && out.data.own).toBe(true);
  });
});

describe("profile loader, a visitor who leaves", () => {
  it("does not read a profile for a request already aborted when the loader starts", async () => {
    let seen: AbortSignal | undefined;
    net.fetchProfile.mockImplementation((_address: string, opts: { signal?: AbortSignal }) => {
      seen = opts.signal;
      return new Promise((_resolve, reject) => {
        if (opts.signal?.aborted) reject(new DOMException("aborted", "AbortError"));
        opts.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
      });
    });
    const controller = new AbortController();
    controller.abort();

    await expect(
      profileLoader(request(`/profile/${A}`, undefined, { signal: controller.signal }), { address: A }),
    ).rejects.toBeTruthy();
    expect(seen).toBeUndefined();
    expect(net.fetchProfile).not.toHaveBeenCalled();
  });

  it("cancels the profile read when the request is aborted", async () => {
    let seen: AbortSignal | undefined;
    net.fetchProfile.mockImplementation(
      (_address: string, opts: { signal?: AbortSignal }) =>
        new Promise((_resolve, reject) => {
          seen = opts.signal;
          opts.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
        }),
    );
    const controller = new AbortController();

    const pending = profileLoader(request(`/profile/${A}`, undefined, { signal: controller.signal }), { address: A });
    await vi.waitFor(() => expect(seen).toBeDefined());
    controller.abort();

    await expect(pending).rejects.toBeTruthy();
    expect(seen?.aborted).toBe(true);
  });
});

describe("profile document request, through the router's handler", () => {
  const handler = createStaticHandler([
    {
      id: "profile",
      path: "/profile/:address/:tab?",
      loader: ({ request, params }) => profileLoader(request, params),
      ErrorBoundary: () => null,
    },
  ]);

  it.each([
    ["an address that cannot be a wallet", "/profile/not-a-wallet", () => undefined, 404],
    [
      "a failing profile service",
      `/profile/${A}`,
      () => net.fetchProfile.mockRejectedValue(new CatalystError("Catalyst unreachable", "/lambdas/profile", 0)),
      503,
    ],
  ])("answers %s with the loader's status", async (_case, path, arrange, status) => {
    arrange();

    const context = await handler.query(new Request(`http://localhost${path}`));

    expect(context).not.toBeInstanceOf(Response);
    expect((context as { statusCode: number }).statusCode).toBe(status);
  });

  it("answers 200 with the dehydrated profile for a wallet", async () => {
    const context = (await handler.query(new Request(`http://localhost/profile/${A}`))) as {
      statusCode: number;
      loaderData: Record<string, ProfileLoaderData>;
    };

    expect(context.statusCode).toBe(200);
    expect(context.loaderData.profile?.dehydratedState.queries[0]?.queryKey).toEqual(profileKeys.profile(A));
  });
});

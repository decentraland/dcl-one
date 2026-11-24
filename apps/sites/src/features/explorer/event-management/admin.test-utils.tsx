import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { Fragment, StrictMode, type ReactElement } from "react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError, postJSON, signedGetJSON } from "@/data/lib/catalyst/client";
import type { EventProfile, ManagedEvent } from "@/data/lib/catalyst/places/event-management";

export const MODERATOR = "0x1111111111111111111111111111111111111111";
/** The creator of the fixture events: another wallet than the signed-in one. */
export const CREATOR = "0x5555555555555555555555555555555555555555";

export function makeIdentity(signer = MODERATOR): AuthIdentity {
  return { signer, ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
}

/** A complete Events API record, so the real loaders parse it. */
export function makeEvent(overrides: Partial<ManagedEvent> = {}): ManagedEvent {
  return {
    id: "pending-one", name: "Moon concert", image: "https://example.com/cover.png", image_vertical: null, description: "A concert",
    start_at: "2099-01-01T18:00:00Z", finish_at: "2099-01-01T20:00:00Z", next_start_at: null, next_finish_at: null, duration: null,
    all_day: false, x: 0, y: 0, server: "moon.dcl.eth", url: null, user: CREATOR, user_name: "Luna",
    estate_id: null, estate_name: null, scene_name: null, approved: false, rejected: false,
    highlighted: false, trending: false, world: true, recurrent: false, recurrent_frequency: null,
    recurrent_weekday_mask: 0, recurrent_month_mask: 0, recurrent_interval: 1, recurrent_setpos: null,
    recurrent_monthday: null, recurrent_count: null, recurrent_until: null, recurrent_dates: [],
    categories: [], schedules: [], total_attendees: 0, latest_attendees: [], coordinates: [0, 0],
    position: [0, 0], live: false, attending: false, place_id: null, community_id: null,
    featured_item: null, created_at: null, updated_at: new Date().toISOString(), approved_by: null, rejected_by: null,
    rejection_reason: null, deleted_by_user: false, deleted_by_admin: false, deleted_by: null,
    deleted_at: null, deleted_reason: null, previous_place_id: null,
    ...overrides,
  };
}

export type FakeEventsApi = {
  profile: EventProfile;
  /** The moderator-only list; `"forbidden"` answers 403 as for an account that is not a moderator. */
  profiles: EventProfile[] | "forbidden";
  events: ManagedEvent[];
  /**
   * Other wallets' stored settings, as one wallet's read answers them. A wallet not named here answers its
   * grants in `profiles`, as the server stores one set for both reads, or none when it is not listed.
   */
  settings?: Record<string, EventProfile>;
  /** Fails the next request of each kind once, with this error. */
  failNext: Partial<Record<"profile" | "events" | "moderate" | "settings" | "save", Error>>;
};

/**
 * Answers the Events API at the Catalyst client boundary (the test mocks `signedGetJSON` and `postJSON`
 * of `@/data/lib/catalyst/client` with `vi.fn()`), so the real loaders, parsers and queries run.
 */
export function fakeEventsApi(api: FakeEventsApi) {
  const take = (kind: keyof FakeEventsApi["failNext"]) => {
    const error = api.failNext[kind];
    delete api.failNext[kind];
    if (error) throw error;
  };
  vi.mocked(signedGetJSON).mockImplementation(async (path: string) => {
    if (path === "/events/api/profiles/me/settings") {
      take("profile");
      return { ok: true, data: api.profile } as never;
    }
    if (path === "/events/api/profiles/settings") {
      if (api.profiles === "forbidden") throw new CatalystError("Forbidden", path, 403);
      return { ok: true, data: api.profiles } as never;
    }
    if (path === "/events/api/events") {
      take("events");
      return { ok: true, data: api.events } as never;
    }
    const wallet = /^\/events\/api\/profiles\/(0x[0-9a-f]{40})\/settings$/.exec(path);
    if (wallet) {
      take("settings");
      const listed = Array.isArray(api.profiles) ? api.profiles.find((entry) => entry.user.toLowerCase() === wallet[1]) : undefined;
      return { ok: true, data: api.settings?.[wallet[1]!] ?? listed ?? null } as never;
    }
    throw new Error(`Unexpected read ${path}`);
  });
  vi.mocked(postJSON).mockImplementation(async (path: string, body: unknown) => {
    const moderated = /^\/events\/api\/events\/([^/]+)$/.exec(path);
    if (moderated) {
      take("moderate");
      return { ok: true, data: moderate(api, decodeURIComponent(moderated[1]!), body as { approved?: boolean; rejected?: boolean }) } as never;
    }
    const profile = /^\/events\/api\/profiles\/([^/]+)\/settings$/.exec(path);
    if (profile) {
      take("save");
      return { ok: true, data: { user: profile[1], permissions: (body as { permissions: string[] }).permissions } } as never;
    }
    throw new Error(`Unexpected write ${path}`);
  });
}

/**
 * The API's signed event PATCH (explore `events/writes.rs`, `patch_event_as_wallet` and `update_event`):
 * another creator's event refuses a wallet without `edit_any_event`; `approved` applies from an
 * `approve_any_event` grant, or the owner's `approve_own_event`; `rejected` from the owner or an
 * `edit_any_event` grant. A field the account may not set is ignored, as the API ignores it.
 */
function moderate(api: FakeEventsApi, id: string, patch: { approved?: boolean; rejected?: boolean }) {
  const event = api.events.find((candidate) => candidate.id === id);
  if (!event) throw new CatalystError("Not found", id, 404);
  const has = (permission: string) => api.profile.permissions.includes(permission);
  const owner = (event.user ?? "").toLowerCase() === api.profile.user.toLowerCase();
  if (!owner && !has("edit_any_event")) throw new CatalystError("You don't have permission to edit this event", id, 403);
  const approves = has("approve_any_event") || (owner && has("approve_own_event"));
  const rejects = owner || has("edit_any_event");
  const rejected = rejects && patch.rejected !== undefined ? patch.rejected : event.rejected;
  const approved = approves && patch.approved !== undefined ? patch.approved : event.approved;
  return { approved: rejected ? false : approved, rejected };
}

/** How many reads of one Events API path were made. */
export const readsOf = (path: string) => vi.mocked(signedGetJSON).mock.calls.filter(([called]) => called === path).length;

function Location() {
  const location = useLocation();
  return <output aria-label="Location">{location.pathname + location.search}</output>;
}

/** The router location the page is at, as the probe beside it shows it. */
export const currentPath = () => screen.getByRole("status", { name: "Location" }).textContent;

/**
 * Renders a page at `path` beside a /events probe, under a fresh query client. `strict` wraps it in
 * StrictMode, as the Sites client entry renders it, so effects run, clean up and run again as in
 * development; it also repeats the page's reads, so tests that count reads leave it off.
 */
export function renderAdminPage(page: ReactElement, path: string, { strict = false } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const Wrap = strict ? StrictMode : Fragment;
  const view = render(
    <Wrap>
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/events" element={<p>Events home</p>} />
          <Route path="/events/edit-event/:id" element={<p>Edit page</p>} />
          <Route path="*" element={page} />
        </Routes>
        <Location />
      </MemoryRouter>
    </QueryClientProvider>
    </Wrap>,
  );
  return { client, view };
}

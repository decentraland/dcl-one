import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";

import type { AuthIdentity } from "../auth/types";
import {
  fetchCommunity,
  fetchCommunityEvents,
  fetchCommunityMembers,
  fetchJoinRequests,
  MEMBERS_PAGE_SIZE,
  EVENTS_PAGE_SIZE,
  type EventsPage,
  type MembersPage,
} from "../catalyst/social/community-detail";

/** The Sites root default, declared here so a loader's dehydrated data and the page agree on freshness. */
export const COMMUNITY_STALE_MS = 30_000;

const account = (identity: AuthIdentity) => identity.signer.toLowerCase();

/**
 * Public keys are `["community", part, id]`. What depends on who reads it (the member's role, a
 * private community's members, the member's requests) sits under `["private", account, "community", ...]`.
 */
export const communityKeys = {
  all: ["community"] as const,
  detail: (id: string) => ["community", "detail", id] as const,
  members: (id: string) => ["community", "members", id] as const,
  events: (id: string) => ["community", "events", id] as const,
  viewer: (signer: string, id: string) => ["private", signer.toLowerCase(), "community", "detail", id] as const,
  viewerMembers: (signer: string, id: string) =>
    ["private", signer.toLowerCase(), "community", "members", id] as const,
  requests: (signer: string) => ["private", signer.toLowerCase(), "community", "requests"] as const,
};

export const communityQuery = (id: string) =>
  queryOptions({
    queryKey: communityKeys.detail(id),
    staleTime: COMMUNITY_STALE_MS,
    queryFn: ({ signal }) => fetchCommunity(id, { signal }),
  });

/** The community read as the signed-in member: the same record plus their role. */
export const viewerCommunityQuery = (identity: AuthIdentity, id: string) =>
  queryOptions({
    queryKey: communityKeys.viewer(account(identity), id),
    staleTime: COMMUNITY_STALE_MS,
    queryFn: ({ signal }) => fetchCommunity(id, { identity, signal }),
  });

const nextOffset = (size: number) => (last: { hasMore: boolean }, pages: unknown[]) =>
  last.hasMore ? pages.length * size : undefined;

/**
 * One community's members, a page at a time. With an identity they are read signed, as that member
 * (a private community shows them to members only) and cached under the member's account.
 */
export const membersQuery = (id: string, identity?: AuthIdentity | null) =>
  infiniteQueryOptions({
    queryKey: identity ? communityKeys.viewerMembers(account(identity), id) : communityKeys.members(id),
    staleTime: COMMUNITY_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }): Promise<MembersPage> =>
      fetchCommunityMembers(id, pageParam, { identity, signal }),
    getNextPageParam: nextOffset(MEMBERS_PAGE_SIZE),
  });

export const eventsQuery = (id: string) =>
  infiniteQueryOptions({
    queryKey: communityKeys.events(id),
    staleTime: COMMUNITY_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }): Promise<EventsPage> => fetchCommunityEvents(id, pageParam, { signal }),
    getNextPageParam: (last: EventsPage, pages: EventsPage[]) => {
      const loaded = pages.reduce((sum, page) => sum + page.events.length, 0);
      return last.events.length > 0 && loaded < last.total ? pages.length * EVENTS_PAGE_SIZE : undefined;
    },
  });

export const joinRequestsQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: communityKeys.requests(account(identity)),
    staleTime: COMMUNITY_STALE_MS,
    queryFn: ({ signal }) => fetchJoinRequests(identity, { signal }),
  });

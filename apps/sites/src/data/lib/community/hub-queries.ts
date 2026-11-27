import { infiniteQueryOptions, queryOptions, skipToken, type QueryClient } from "@tanstack/react-query";

import type { AuthIdentity } from "../auth/types";
import {
  fetchCommunityRequests,
  fetchHubCommunities,
  fetchHubDetail,
  fetchHubPosts,
  fetchOwnRequests,
  HUB_POSTS_PAGE_SIZE,
  type HubCommunity,
  type HubPage,
  type HubPost,
} from "../catalyst/social/community-hub";

import { COMMUNITY_STALE_MS } from "./queries";

/**
 * Every hub read depends on who reads it (each row carries the reader's role), so a member's reads
 * sit under `["private", account, "community", "hub", ...]` and a visitor's under `["community", "hub", ...]`.
 */
const scope = (viewer: string | null) =>
  viewer ? (["private", viewer.toLowerCase(), "community", "hub"] as const) : (["community", "hub"] as const);

export const hubKeys = {
  all: (viewer: string | null) => scope(viewer),
  communities: (viewer: string | null) => [...scope(viewer), "communities"] as const,
  search: (viewer: string | null, term: string) => [...scope(viewer), "communities", "search", term] as const,
  detail: (viewer: string | null, id: string) => [...scope(viewer), "detail", id] as const,
  posts: (viewer: string | null, id: string) => [...scope(viewer), "posts", id] as const,
  latestPost: (viewer: string | null, id: string) => [...scope(viewer), "posts", id, "latest"] as const,
  ownRequests: (viewer: string) => [...scope(viewer), "requests", "own"] as const,
  communityRequests: (viewer: string, id: string) => [...scope(viewer), "requests", "community", id] as const,
};

const viewerOf = (identity: AuthIdentity | null) => identity?.signer.toLowerCase() ?? null;

/** The next offset: the rows already received, until the API's total is reached. */
function nextOffset<T>(last: HubPage<T>, pages: HubPage<T>[]): number | undefined {
  const received = pages.reduce((sum, page) => sum + page.received, 0);
  return last.received > 0 && received < last.total ? received : undefined;
}

/** Every community the reader can discover, or those matching `term`, a page at a time. */
export const hubCommunitiesQuery = (identity: AuthIdentity | null, term = "") =>
  infiniteQueryOptions({
    queryKey: term ? hubKeys.search(viewerOf(identity), term) : hubKeys.communities(viewerOf(identity)),
    staleTime: COMMUNITY_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }) => fetchHubCommunities({ identity, signal, offset: pageParam, search: term || undefined }),
    getNextPageParam: nextOffset<HubCommunity>,
  });

export const hubDetailQuery = (identity: AuthIdentity | null, id: string) =>
  queryOptions({
    queryKey: hubKeys.detail(viewerOf(identity), id),
    staleTime: COMMUNITY_STALE_MS,
    queryFn: ({ signal }) => fetchHubDetail(id, { identity, signal }),
  });

/** The community's latest announcement, for its overview; disabled when the reader cannot read them. */
export const hubLatestPostQuery = (identity: AuthIdentity | null, id: string, enabled: boolean) =>
  queryOptions({
    queryKey: hubKeys.latestPost(viewerOf(identity), id),
    staleTime: COMMUNITY_STALE_MS,
    queryFn: enabled
      ? async ({ signal }): Promise<HubPost | null> =>
          (await fetchHubPosts(id, { identity, signal, offset: 0, limit: 1 })).rows[0] ?? null
      : skipToken,
  });

export const hubPostsQuery = (identity: AuthIdentity | null, id: string) =>
  infiniteQueryOptions({
    queryKey: hubKeys.posts(viewerOf(identity), id),
    staleTime: COMMUNITY_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }) => fetchHubPosts(id, { identity, signal, offset: pageParam, limit: HUB_POSTS_PAGE_SIZE }),
    getNextPageParam: nextOffset<HubPost>,
  });

/** The member's own invitations and requests; a visitor has none to read. */
export const ownRequestsQuery = (identity: AuthIdentity | null) =>
  queryOptions({
    queryKey: identity ? hubKeys.ownRequests(viewerOf(identity)!) : [...hubKeys.all(null), "requests", "own"],
    staleTime: COMMUNITY_STALE_MS,
    queryFn: identity ? ({ signal }) => fetchOwnRequests(identity, signal) : skipToken,
  });

/** The requests a community's owner or moderators decide on; disabled for anyone else. */
export const communityRequestsQuery = (identity: AuthIdentity | null, id: string, enabled: boolean) =>
  queryOptions({
    queryKey: identity ? hubKeys.communityRequests(viewerOf(identity)!, id) : [...hubKeys.all(null), "requests", "community", id],
    staleTime: COMMUNITY_STALE_MS,
    queryFn: identity && enabled ? ({ signal }) => fetchCommunityRequests(identity, id, signal) : skipToken,
  });

/** After a member joins, asks to join, leaves or answers a request: everything the hub shows them. */
export function invalidateHub(client: QueryClient, viewer: string | null): Promise<void> {
  return client.invalidateQueries({ queryKey: hubKeys.all(viewer) });
}

/** After a member's role changes or they are removed: the lists' counts and that community. */
export function invalidateHubMembers(client: QueryClient, viewer: string | null, id: string): Promise<unknown> {
  return Promise.all([
    client.invalidateQueries({ queryKey: hubKeys.communities(viewer) }),
    client.invalidateQueries({ queryKey: hubKeys.detail(viewer, id), exact: true }),
  ]);
}

/** After an announcement is published, edited, deleted or liked: the feed and the overview's latest one. */
export function invalidateHubPosts(client: QueryClient, viewer: string | null, id: string): Promise<void> {
  return client.invalidateQueries({ queryKey: hubKeys.posts(viewer, id) });
}

import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { createCommunity } from "@/data/lib/catalyst/social/community-commit";
import {
  changeMember,
  decideRequest,
  deletePost,
  editPost,
  inviteMember,
  leaveCommunity,
  likePost,
  publishPost,
  type HubCommunity,
  type HubMember,
  type HubPage,
  type HubPost,
  type HubRequest,
  type HubRequestDecision,
} from "@/data/lib/catalyst/social/community-hub";
import { joinCommunity, requestToJoin } from "@/data/lib/catalyst/social/community-detail";
import { emptyDraft } from "@/data/lib/catalyst/social/create-community";
import {
  communityRequestsQuery,
  hubCommunitiesQuery,
  hubDetailQuery,
  hubKeys,
  hubLatestPostQuery,
  hubPostsQuery,
  invalidateHub,
  invalidateHubMembers,
  invalidateHubPosts,
  ownRequestsQuery,
} from "@/data/lib/community/hub-queries";
import { invalidateAfterMembershipChange } from "@/data/lib/community/queries";

import { canReadPosts, failureText, manages } from "./access";

const SEARCH_DEBOUNCE_MS = 250;

const viewerOf = (identity: AuthIdentity | null) => identity?.signer.toLowerCase() ?? null;

/** The pages' rows, once each: a row that moved between pages while they loaded shows once. */
function rowsOf<T extends { id: string }>(pages: HubPage<T>[] | undefined): T[] {
  return [...new Map((pages ?? []).flatMap((page) => page.rows).map((row) => [row.id, row])).values()];
}

/** The communities the reader can discover, and those matching what they type (read a moment after they stop). */
export function useHubDirectory(identity: AuthIdentity | null) {
  const [search, setSearch] = useState("");
  const [term, setTerm] = useState("");
  useEffect(() => {
    const timer = setTimeout(() => setTerm(search.trim()), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [search]);
  const list = useInfiniteQuery(hubCommunitiesQuery(identity));
  const matches = useInfiniteQuery({ ...hubCommunitiesQuery(identity, term), enabled: term !== "" });
  const discovery = term ? matches : list;
  const communities = useMemo(() => rowsOf(list.data?.pages), [list.data]);
  const found = useMemo(() => rowsOf(discovery.data?.pages), [discovery.data]);
  return {
    communities,
    pending: list.isPending,
    failed: list.isError,
    retry: () => void list.refetch(),
    search,
    setSearch,
    discovery: {
      communities: found,
      pending: search.trim() !== term || discovery.isPending,
      failed: discovery.isError,
      retry: () => void discovery.refetch(),
      loadingMore: discovery.isFetchingNextPage,
      loadMore: discovery.hasNextPage ? () => void discovery.fetchNextPage() : undefined,
    },
  };
}

/** One community as the hub opens it, with its latest announcement when the reader may read them. */
export function useHubCommunity(identity: AuthIdentity | null, id: string | null) {
  const detail = useQuery({ ...hubDetailQuery(identity, id ?? ""), enabled: id !== null });
  const community = detail.data?.community ?? null;
  const readable = id !== null && canReadPosts(community);
  const latest = useQuery(hubLatestPostQuery(identity, id ?? "", readable));
  return {
    detail: detail.data,
    community,
    pending: id !== null && detail.isPending,
    failed: detail.isError,
    managing: manages(community?.role),
    readable,
    latestPost: readable && !latest.isError ? (latest.data ?? null) : null,
    latestPending: readable && latest.isPending,
  };
}

type MembershipWrite = { community: HubCommunity; kind: "join" | "leave" };

/**
 * Joins a public community, asks to join a private one, or leaves. Everything the hub shows the
 * member is read again afterwards, and so is that community's own page.
 */
export function useHubMembership(identity: AuthIdentity | null) {
  const client = useQueryClient();
  const viewer = viewerOf(identity);
  const mutation = useMutation({
    mutationFn: async ({ community, kind }: MembershipWrite) => {
      if (!identity) throw new Error("Sign in to manage membership.");
      if (kind === "leave") await leaveCommunity(identity, community.id);
      else if (community.privacy === "private") await requestToJoin(identity, community.id);
      else await joinCommunity(identity, community.id);
      await Promise.all([
        invalidateHub(client, viewer),
        invalidateAfterMembershipChange(client, identity.signer, community.id, community.privacy === "private" && kind === "join" ? "request" : "join"),
      ]);
    },
  });
  return {
    pending: mutation.isPending ? mutation.variables.kind : null,
    error: mutation.error ? failureText(mutation.error, "Your membership could not be changed. Try again.") : undefined,
    reset: mutation.reset,
    change: (write: MembershipWrite) => mutation.mutateAsync(write),
  };
}

/**
 * The member's own invitations and requests and, for a community they manage, the requests to
 * decide on, with the answers and the invitations they send.
 */
export function useHubRequests(identity: AuthIdentity | null, communityId: string | null, managing: boolean) {
  const client = useQueryClient();
  const viewer = viewerOf(identity);
  const own = useQuery(ownRequestsQuery(identity));
  const managed = useQuery(communityRequestsQuery(identity, communityId ?? "", managing && communityId !== null));
  const decide = useMutation({
    mutationFn: async ({ row, intention }: { row: HubRequest; intention: HubRequestDecision }) => {
      if (!identity) throw new Error("Sign in to answer this request.");
      await decideRequest(identity, row, intention);
      await Promise.all([
        invalidateHub(client, viewer),
        invalidateAfterMembershipChange(client, identity.signer, row.communityId, "join"),
      ]);
    },
  });
  const invite = useMutation({
    mutationFn: async (address: string) => {
      if (!identity || !communityId) throw new Error("Sign in to invite members.");
      await inviteMember(identity, communityId, address);
      await invalidateHub(client, viewer);
    },
  });
  const pendingOwn = useMemo(() => (own.data ?? []).filter((row) => row.status === "pending"), [own.data]);
  const state = (query: typeof own, management: boolean) => {
    const failure = query.error ?? decide.error ?? (management ? invite.error : null);
    return {
    rows: query.data ?? [],
    pending: query.isPending && query.fetchStatus !== "idle",
    error: failure
      ? failureText(failure, query.error ? "Membership requests could not be loaded." : "The request could not be sent. Try again.")
      : undefined,
    busy: decide.isPending || invite.isPending,
    retry: () => {
      decide.reset();
      invite.reset();
      void query.refetch();
    },
    decide: (row: HubRequest, intention: HubRequestDecision) => {
      if (!decide.isPending) decide.mutate({ row, intention });
    },
    };
  };
  return {
    pendingOwn,
    own: state(own, false),
    managed: {
      ...state(managed, true),
      invite: (address: string) => {
        if (!invite.isPending) invite.mutate(address);
      },
    },
    /** The member asked to join this community and has not been answered. */
    requested: (id: string) => pendingOwn.some((row) => row.communityId === id && row.type === "request_to_join"),
  };
}

const POSTS_REFUSAL = "You no longer have permission to change posts in this community.";

function newPublishKey(): string {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * The community's announcements, oldest page first on request, and the writes that change them. A
 * publish retried with the same words after a lost answer reuses its key, so it publishes once.
 */
export function useHubPosts(identity: AuthIdentity | null, id: string) {
  const client = useQueryClient();
  const viewer = viewerOf(identity);
  const feed = useInfiniteQuery(hubPostsQuery(identity, id));
  const { refetch } = feed;
  const publishKey = useRef<{ content: string; key: string } | null>(null);
  const posts = useMemo(() => (feed.isError ? [] : rowsOf(feed.data?.pages)), [feed.isError, feed.data]);

  const write = useCallback(
    async (run: (identity: AuthIdentity) => Promise<void>) => {
      if (!identity) throw new Error("Sign in to change posts.");
      try {
        await run(identity);
      } catch (error) {
        void refetch();
        throw new Error(failureText(error, "The post could not be updated. Try again.", POSTS_REFUSAL), { cause: error });
      }
      await invalidateHubPosts(client, viewer, id);
    },
    [identity, refetch, client, viewer, id],
  );

  return {
    posts,
    pending: feed.isPending,
    error: feed.isError ? failureText(feed.error, "Announcements could not be loaded.") : undefined,
    retry: () => void feed.refetch(),
    hasMore: !feed.isError && feed.hasNextPage,
    loadingMore: feed.isFetchingNextPage,
    loadMore: () => void feed.fetchNextPage(),
    publish: async (content: string) => {
      if (publishKey.current?.content !== content) publishKey.current = { content, key: newPublishKey() };
      const { key } = publishKey.current;
      await write((signed) => publishPost(signed, id, content, key));
      publishKey.current = null;
    },
    edit: (post: Pick<HubPost, "id">, content: string) => write((signed) => editPost(signed, id, post.id, content)),
    remove: (post: Pick<HubPost, "id">) => write((signed) => deletePost(signed, id, post.id)),
    /** Likes the post, or takes the like back when the member already likes it. */
    like: (post: Pick<HubPost, "id"> & { isLikedByUser?: boolean }) =>
      write((signed) => likePost(signed, id, post.id, !post.isLikedByUser)),
  };
}

const MEMBER_REFUSAL = "You don't have permission to change this member.";

/** Makes a member a moderator or a member again, or removes them; the counts and the community are read again. */
export function useHubMemberChange(identity: AuthIdentity | null, id: string) {
  const client = useQueryClient();
  const viewer = viewerOf(identity);
  return useCallback(
    async (member: HubMember, change: "moderator" | "member" | "remove") => {
      if (!identity) throw new Error("Sign in to manage members.");
      try {
        await changeMember(identity, id, member.memberAddress, change);
      } catch (error) {
        throw new Error(failureText(error, "The member could not be updated. Try again.", MEMBER_REFUSAL), { cause: error });
      }
      await invalidateHubMembers(client, viewer, id);
    },
    [identity, id, client, viewer],
  );
}

/** Creates a community from the hub's dialog; the lists are read again so it shows at once. */
export function useHubCreate(identity: AuthIdentity | null) {
  const client = useQueryClient();
  const viewer = viewerOf(identity);
  return useCallback(
    async (input: { name: string; description: string; privacy: "public" | "private"; visibility: "all" | "unlisted" }) => {
      if (!identity) throw new Error("Sign in to create a community.");
      const created = await createCommunity({ ...emptyDraft(), ...input }, null, { identity });
      if (!created.id) throw new Error("The server did not identify the created community.");
      await client.refetchQueries({ queryKey: hubKeys.communities(viewer), exact: true });
      return created;
    },
    [identity, client, viewer],
  );
}

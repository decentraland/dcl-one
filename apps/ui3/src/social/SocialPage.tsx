import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import ChromeShell from "../components/ChromeShell";
import DclTopBar from "../web/frames/DclTopBar";
import Communities, { type SocialCommunity, type SocialDetail, type SocialMember, type SocialPost, type SocialPlace } from "../explorer/pages/Communities";
import CommunityCreate from "../explorer/components/CommunityCreate";
import ConfirmDialog from "../explorer/components/ConfirmDialog";
import CommunityRequests, { type CommunityRequest, type RequestDecision } from "./CommunityRequests";
import CommunityPosts from "./CommunityPosts";
import CommunityMemberActions from "./CommunityMemberActions";
import { eventXY, fetchEvents } from "../data/catalyst/events";
import { PLAY_EVENTS_PARAMS } from "../data/screens/play";
import { publicThumbnail } from "../data/thumbnail";
import "../web/frames/sitelayout.css";
import "./social-page.css";

type Props = {
  viewer: string | null;
  communityId: string | null;
  postId?: string | null;
  request: (path: string, init?: RequestInit) => Promise<Response>;
  onSelect: (id: string | null) => void;
  onSignIn: () => void;
};
type CommunityPage = { results: SocialCommunity[]; total: number };
const nextCommunityPage = (last: CommunityPage, pages: CommunityPage[]) => {
  const offset = pages.reduce((count, page) => count + page.results.length, 0);
  return last.results.length && offset < last.total ? offset : undefined;
};
const communityRows = (pages: CommunityPage[] = []) => [...new Map(pages.flatMap(page => page.results).map(row => [row.id, row])).values()];

export default function SocialPage({ viewer, communityId, postId, request, onSelect, onSignIn }: Props) {
  const [creating, setCreating] = useState(false);
  const [joining, setJoining] = useState<SocialCommunity | null>(null);
  const [search, setSearch] = useState("");
  const [term, setTerm] = useState("");
  useEffect(() => { const timer = setTimeout(() => setTerm(search.trim()), 250); return () => clearTimeout(timer); }, [search]);
  const client = useQueryClient();
  async function read<T,>(path: string, init?: RequestInit): Promise<T> {
    const response = await request(path, init);
    if (response.status === 204) return undefined as T;
    const text = await response.text();
    const body = text ? JSON.parse(text) : null;
    if (!response.ok) throw Object.assign(new Error(body?.message || `Community request failed (${response.status})`), { status: response.status });
    return (body?.data ?? body) as T;
  }
  async function readRequests(path: string, signal: AbortSignal) {
    const rows: CommunityRequest[] = [];
    for (;;) {
      const page = await read<{ results: CommunityRequest[]; total: number }>(`${path}?limit=100&offset=${rows.length}`, { signal });
      rows.push(...page.results);
      if (rows.length >= page.total || !page.results.length) return rows;
    }
  }
  const refresh = () => client.invalidateQueries({ queryKey: ["web-social", viewer] });
  const ownRequests = useQuery({
    queryKey: ["web-social", viewer, "requests"], enabled: !!viewer, retry: false,
    queryFn: ({ signal }) => readRequests(`/v1/members/${viewer}/requests`, signal),
  });
  const list = useInfiniteQuery({
    queryKey: ["web-social", viewer, "communities"],
    initialPageParam: 0, getNextPageParam: nextCommunityPage,
    queryFn: ({ pageParam, signal }) => read<CommunityPage>(`/v1/communities?limit=100&offset=${pageParam}`, { signal }),
    retry: false,
  });
  const matches = useInfiniteQuery({
    queryKey: ["web-social", viewer, "communities", "search", term], enabled: !!term, retry: false,
    initialPageParam: 0, getNextPageParam: nextCommunityPage,
    queryFn: ({ pageParam, signal }) => read<CommunityPage>(`/v1/communities?limit=100&offset=${pageParam}&search=${encodeURIComponent(term)}`, { signal }),
  });
  const discovery = term ? matches : list;
  const live = useQuery({
    queryKey: ["web-social", "live-events"], retry: false, staleTime: 30_000,
    queryFn: ({ signal }) => fetchEvents(PLAY_EVENTS_PARAMS, { signal }),
  });
  const liveEvents = (live.data?.data ?? []).filter(event => event.live).map(event => ({
    id: event.id, name: event.name ?? "Untitled event", image: publicThumbnail(event.image, 320),
    startAt: event.next_start_at ?? event.start_at, attendees: event.total_attendees, live: event.live,
  }));
  const jumpToEvent = (id: string) => {
    const event = live.data?.data.find(row => row.id === id);
    if (!event) return;
    const { x, y } = eventXY(event);
    location.assign(event.world ? "/events" : `/play/?position=${x},${y}`);
  };
  const detail = useQuery({
    queryKey: ["web-social", viewer, communityId],
    enabled: !!communityId,
    retry: false,
    queryFn: async ({ signal }): Promise<SocialDetail> => {
      const path = `/v1/communities/${encodeURIComponent(communityId!)}`;
      const community = await read<SocialCommunity>(path, { signal }).catch(async error => {
        if ([401, 403, 404].includes(error.status)) {
          const summary = await read<{ results: SocialCommunity[] }>(`/v1/communities?communityId=${encodeURIComponent(communityId!)}&limit=1`, { signal });
          return summary.results.find(row => row.id === communityId) ?? null;
        }
        throw error;
      });
      if (!community) return { id: communityId!, community: null, members: [], posts: [], places: [] };
      if (community.privacy === "private" && (!community.role || community.role === "none")) {
        return { id: communityId!, community, members: [], posts: [], places: [] };
      }
      const [members, places] = await Promise.all([
        read<{ results: SocialMember[] }>(`${path}/members?limit=100`, { signal }),
        read<{ places: SocialPlace[] }>(`${path}/places`, { signal }),
      ]);
      return { id: communityId!, community, members: members.results, posts: [], places: places.places ?? [] };
    },
  });
  const membership = useMutation({
    mutationFn: async ({ community, leave }: { community: SocialCommunity; leave?: boolean }) => {
      if (!viewer) throw new Error("Sign in to manage membership.");
      const path = `/v1/communities/${encodeURIComponent(community.id)}`;
      const privateRequest = !leave && community.privacy === "private";
      await read(privateRequest ? `${path}/requests` : leave ? `${path}/members/${viewer}` : `${path}/members`, {
        method: leave ? "DELETE" : "POST",
        ...(privateRequest ? { headers: { "Content-Type": "application/json" }, body: JSON.stringify({ type: "request_to_join" }) } : {}),
      });
      await refresh();
      setJoining(null);
    },
  });
  const community = detail.data?.community;
  const canReadPosts = !!community && community.role !== "banned" && (community.privacy !== "private" || ["owner", "moderator", "mod", "member"].includes(community.role ?? ""));
  const latestPost = useQuery({
    queryKey: ["web-social", viewer, communityId, "posts", "latest"], enabled: canReadPosts, retry: false,
    queryFn: ({ signal }) => read<{ posts: SocialPost[] }>(`/v1/communities/${communityId}/posts?limit=1`, { signal }),
  });
  const managing = ["owner", "moderator", "mod"].includes(community?.role ?? "");
  const communityRequests = useQuery({
    queryKey: ["web-social", viewer, communityId, "requests"], enabled: !!communityId && managing, retry: false,
    queryFn: ({ signal }) => readRequests(`/v1/communities/${communityId}/requests`, signal),
  });
  const decide = useMutation({
    mutationFn: async ({ row, intention }: { row: CommunityRequest; intention: RequestDecision }) => {
      await read(`/v1/communities/${row.communityId}/requests/${row.id}`, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ intention }) });
      await refresh();
    },
  });
  const invite = useMutation({
    mutationFn: async (address: string) => {
      await read(`/v1/communities/${communityId}/requests`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ type: "invite", targetedAddress: address }) });
      await refresh();
    },
  });
  const pendingRequests = (ownRequests.data ?? []).filter(row => row.status === "pending");
  const requestPanel = (management: boolean) => {
    const query = management ? communityRequests : ownRequests;
    return <CommunityRequests key={management ? communityId : viewer} requests={query.data ?? []} managing={management}
      loading={query.isPending} error={query.error?.message || decide.error?.message || (management ? invite.error?.message : undefined)}
      busy={decide.isPending || invite.isPending} onRetry={() => { decide.reset(); invite.reset(); void query.refetch(); }}
      onDecide={(row, intention) => { if (!decide.isPending) decide.mutate({ row, intention }); }} onOpen={onSelect}
      onInvite={management ? address => { if (!invite.isPending) invite.mutate(address); } : undefined} />;
  };
  return (
    <ChromeShell className="social-web" ariaLabel="Social" subnav={false} topbar={<DclTopBar active="social" />} footer={false}>
      <Communities
        chrome={false}
        communities={communityRows(list.data?.pages)}
        discovery={{ communities: communityRows(discovery.data?.pages), filtered: true,
          isLoading: search.trim() !== term || discovery.isPending, isError: discovery.isError,
          onSearch: setSearch, onRetry: () => void discovery.refetch(), loadingMore: discovery.isFetching,
          onLoadMore: discovery.hasNextPage ? () => { void discovery.fetchNextPage(); } : undefined }}
        isLoading={list.isPending}
        isError={list.isError}
        onRetry={() => void list.refetch()}
        isGuest={!viewer}
        liveEvents={liveEvents}
        liveEventsLoading={live.isPending}
        onEvent={() => location.assign("/events")}
        onJumpEvent={jumpToEvent}
        selfAddress={viewer}
        initialCommunityId={communityId}
        initialChannel={postId ? "announcements" : "summary"}
        detail={communityId ? { id: communityId, community: community ?? null, members: [], places: [], ...detail.data,
          posts: canReadPosts && !latestPost.isError ? latestPost.data?.posts ?? [] : [], postsPending: canReadPosts && latestPost.isPending,
          isPending: detail.isPending, isError: detail.isError } : null}
        membership={{ joined: !!community?.role && community.role !== "none", requested: pendingRequests.some(row => row.communityId === communityId && row.type === "request_to_join"),
          pending: membership.isPending ? membership.variables?.leave ? "leave" : "join" : null,
          error: membership.error?.message }}
        onSelect={id => { membership.reset(); onSelect(id); }}
        onJoin={community => { if (!viewer) onSignIn(); else if (!membership.isPending) { if (community.privacy === "private") setJoining(community); else membership.mutate({ community }); } }}
        onLeave={community => { if (viewer && !membership.isPending) membership.mutate({ community, leave: true }); }}
        onCopyLink={id => void navigator.clipboard.writeText(`${location.origin}/social?community=${encodeURIComponent(id)}`)}
        onCreate={() => viewer ? setCreating(true) : onSignIn()}
        requestsContent={viewer ? requestPanel(false) : undefined}
        requestsCount={pendingRequests.length}
        communityRequestsContent={managing ? requestPanel(true) : undefined}
        memberActions={managing ? member => <CommunityMemberActions member={member} viewer={viewer} role={community?.role}
          write={async (method, address, body) => {
            await read(`/v1/communities/${communityId}/members/${address}`, { method,
              ...(body ? { headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) } : {}) });
            await Promise.all([
              client.invalidateQueries({ queryKey: ["web-social", viewer, "communities"] }),
              client.invalidateQueries({ queryKey: ["web-social", viewer, communityId], exact: true }),
            ]);
          }} /> : undefined}
        announcementsContent={detail.data && <CommunityPosts key={`${viewer}:${communityId}`} detail={detail.data} viewer={viewer} postId={postId}
          load={(offset, signal) => read<{ posts: SocialPost[]; total: number }>(`/v1/communities/${communityId}/posts?limit=50&offset=${offset}`, { signal })}
          write={async (method, suffix, body, headers) => {
            await read(`/v1/communities/${communityId}/posts${suffix}`, { method,
              ...(body || headers ? { headers: { ...(body ? { "Content-Type": "application/json" } : {}), ...headers } } : {}),
              ...(body ? { body: JSON.stringify(body) } : {}) });
            await client.invalidateQueries({ queryKey: ["web-social", viewer, communityId, "posts"] });
          }} />}
      />
      {joining && <ConfirmDialog title="Request to join?" body={membership.error?.message || `Ask the moderators of ${joining.name} to approve your membership.`}
        confirmLabel={membership.isPending ? "Sending…" : "Send request"} cancelLabel="Cancel"
        onConfirm={() => { if (!membership.isPending) membership.mutate({ community: joining }); }} onCancel={() => { if (!membership.isPending) setJoining(null); }} />}
      {creating && <CommunityCreate
        create={async input => {
          const body = new FormData();
          Object.entries(input).forEach(([key, value]) => { if (value !== undefined) body.set(key, value); });
          const result = await read<{ id: string; name: string }>("/v1/communities", { method: "POST", body });
          if (!result.id) throw new Error("The server did not identify the created community.");
          await list.refetch();
          onSelect(result.id);
          return result;
        }}
        onDone={() => setCreating(false)}
      />}
    </ChromeShell>
  );
}

import "@dcl/one-ui/foundations/foundations.css";
import { useEffect, useMemo, useRef, useState } from "react";
import { dehydrate, HydrationBoundary, QueryClient, useInfiniteQuery } from "@tanstack/react-query";
import { Link, useSearchParams } from "react-router";

import SitesChrome from "@ui/web/frames/SitesChrome";
import { CommunityCard, type CommunityCardLinkProps } from "@dcl/one-ui";
import { CommunitiesBrowse, CommunitiesMessage, CommunityGrid } from "@dcl/one-ui/apps/sites/communities/CommunitiesBrowse";
import { PlacesPage } from "@dcl/one-ui/apps/sites/places/PlacesPage";

import { QUERY_CLIENT_CONFIG } from "@/core/lib/query/defaults";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { browseCommunitiesQuery, communityRows, effectiveCommunitySearch } from "@/data/lib/catalyst/social/communities-browse";
import { communityHref } from "@/data/lib/catalyst/social/communities-list";
import { PlacesTabs } from "@/features/places/PlacesTabs";

import type { Route } from "./+types/places_.communities";

const SEARCH_DEBOUNCE_MS = 300;
/** Placeholder cards while the first page loads: one row on the widest grid. */
const SKELETONS = 4;
/** The covers of the first row load eagerly. */
const PRIORITY_CARDS = 4;

const COPY = {
  heading: "Communities",
  grid: "Communities",
  search: "Search communities",
  clear: "Clear search",
  empty: "No communities found.",
  error: "Something went wrong loading communities.",
  retry: "Retry",
  loading: "Loading communities…",
  loadMore: "Load more communities",
};
const CARD_LABELS = { public: "Public", private: "Private", view: "View" };

const COUNT = new Intl.NumberFormat("en-US");
const resultsLabel = (count: number, more: boolean) =>
  count === 0 ? COPY.empty : `${COUNT.format(count)}${more ? "+" : ""} ${count === 1 && !more ? "community" : "communities"}`;
const membersLabel = (count: number | null) =>
  count === null ? undefined : `${COUNT.format(count)} ${count === 1 ? "member" : "members"}`;

export function meta() {
  return [
    { title: "Communities | Decentraland" },
    { name: "description", content: "Find Decentraland communities to join, from builders and artists to music and events." },
  ];
}

/**
 * The first document reads the first page for the URL's search; an unavailable list answers 503 with the
 * page's error state.
 */
export async function loader({ request, context }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request, context);
  const search = effectiveCommunitySearch(new URL(request.url).searchParams.get("search"));
  const client = new QueryClient(QUERY_CLIENT_CONFIG);
  let unavailable = false;
  // The query function reads its own signal; cancelling the query aborts it, and so the upstream read.
  const cancel = () => void client.cancelQueries();
  if (request.signal.aborted) cancel();
  else request.signal.addEventListener("abort", cancel, { once: true });
  try {
    await client.fetchInfiniteQuery(browseCommunitiesQuery(search)).catch(() => {
      // A visitor who left aborted the read: that is not an outage.
      if (!request.signal.aborted) unavailable = true;
    });
  } finally {
    request.signal.removeEventListener("abort", cancel);
  }
  const dehydratedState = dehydrate(client);
  client.clear();
  return wrap({ sid, dehydratedState, unavailable }, { status: unavailable ? 503 : 200 });
}

/** Client navigations read through the browser; the server loader runs for the first document only. */
export async function clientLoader(): Promise<null> {
  return null;
}

export default function PlacesCommunities({ loaderData }: Route.ComponentProps) {
  return (
    <HydrationBoundary state={loaderData?.dehydratedState}>
      <CommunitiesExplorer unavailable={loaderData?.unavailable ?? false} />
    </HydrationBoundary>
  );
}

function CardLink({ href, ...props }: CommunityCardLinkProps) {
  return <Link to={href} {...props} />;
}

function CommunitiesExplorer({ unavailable }: { unavailable: boolean }) {
  const [params, setParams] = useSearchParams();
  const rawSearch = params.get("search") ?? "";
  const search = effectiveCommunitySearch(rawSearch);

  // A document the server answered 503 shows the error at once for its search; Retry, not hydration,
  // reads again, and any other search reads normally.
  const [heldSearch, setHeldSearch] = useState(unavailable ? search : null);
  const held = heldSearch === search;
  useEffect(() => {
    if (heldSearch !== null && heldSearch !== search) setHeldSearch(null);
  }, [heldSearch, search]);

  const list = useInfiniteQuery({ ...browseCommunitiesQuery(search), enabled: !held });
  const rows = useMemo(() => communityRows(list.data?.pages ?? []), [list.data]);

  // Search types into the field at once and reaches the URL (and the list) once typing pauses.
  const [searchInput, setSearchInput] = useState(rawSearch);
  const lastPushed = useRef(rawSearch);
  useEffect(() => {
    // Back and forward bring another search: the field follows the URL.
    if (rawSearch !== lastPushed.current) {
      lastPushed.current = rawSearch;
      setSearchInput(rawSearch);
    }
  }, [rawSearch]);
  useEffect(() => {
    if (searchInput === lastPushed.current) return;
    const timer = setTimeout(() => {
      lastPushed.current = searchInput;
      setParams(
        (previous) => {
          const next = new URLSearchParams(previous);
          if (searchInput.trim()) next.set("search", searchInput);
          else next.delete("search");
          return next;
        },
        { replace: true, preventScrollReset: true },
      );
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [searchInput, setParams]);

  // Announces the loading state, then the outcome of the read or search.
  const status = held ? "" : list.isPending ? COPY.loading : list.isSuccess ? resultsLabel(rows.length, list.hasNextPage) : "";

  return (
    <SitesChrome active="explore">
      <PlacesPage title="Places" tabs={<PlacesTabs active="communities" />} status={status}>
        <CommunitiesBrowse
          heading={COPY.heading}
          search={searchInput}
          onSearch={setSearchInput}
          searchLabel={COPY.search}
          searchPlaceholder={COPY.search}
          clearLabel={COPY.clear}
        >
          {results()}
        </CommunitiesBrowse>
      </PlacesPage>
    </SitesChrome>
  );

  function results() {
    if (held) {
      return <CommunitiesMessage tone="alert" message={COPY.error} action={{ label: COPY.retry, onClick: () => setHeldSearch(null) }} />;
    }
    if (list.isError && !list.data) {
      return <CommunitiesMessage tone="alert" message={COPY.error} action={{ label: COPY.retry, onClick: () => void list.refetch() }} />;
    }
    if (list.isPending) {
      return (
        <CommunityGrid label={COPY.grid} busy>
          {Array.from({ length: SKELETONS }, (_, index) => (
            <li key={index}>
              <CommunityCard loading loadingLabel={COPY.loading} />
            </li>
          ))}
        </CommunityGrid>
      );
    }
    if (!rows.length && !list.hasNextPage) return <CommunitiesMessage message={COPY.empty} />;
    // A first page whose rows were all unusable: nothing to list yet, but more to read.
    if (!rows.length) return <CommunitiesMessage message={COPY.empty} action={loadMore()} />;
    return (
      <>
        <CommunityGrid label={COPY.grid}>
          {rows.map((community, index) => (
            <li key={community.id}>
              <CommunityCard
                name={community.name}
                href={communityHref(community.id)}
                linkComponent={CardLink}
                image={community.thumbnailUrl}
                priority={index < PRIORITY_CARDS}
                ownerName={community.ownerName}
                privacy={community.privacy}
                membersLabel={membersLabel(community.membersCount)}
                description={community.description}
                labels={CARD_LABELS}
              />
            </li>
          ))}
        </CommunityGrid>
        {list.isFetchNextPageError ? (
          <CommunitiesMessage tone="alert" message={COPY.error} action={{ label: COPY.retry, onClick: () => void list.fetchNextPage() }} />
        ) : list.hasNextPage ? (
          <CommunitiesMessage action={loadMore()} />
        ) : null}
      </>
    );
  }

  function loadMore() {
    return { label: COPY.loadMore, loading: list.isFetchingNextPage, onClick: () => void list.fetchNextPage() };
  }
}

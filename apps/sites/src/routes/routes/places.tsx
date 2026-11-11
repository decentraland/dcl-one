import "@dcl/one-ui/foundations/foundations.css";
import { useEffect, useMemo, useRef, useState, useSyncExternalStore, type MouseEvent } from "react";
import { dehydrate, HydrationBoundary, QueryClient } from "@tanstack/react-query";
import { useNavigate, useSearchParams } from "react-router";

import SitesChrome from "@ui/web/frames/SitesChrome";
import { ExploreBand, type ExploreBandLabels } from "@dcl/one-ui/apps/sites/places/ExploreBand";
import { JumpInModal } from "@dcl/one-ui/apps/sites/places/JumpInModal";
import { LiveNow } from "@dcl/one-ui/apps/sites/places/LiveNow";
import { LoadMore, PlaceGrid, PlacesMessage } from "@dcl/one-ui/apps/sites/places/PlaceGrid";
import { PlacesPage } from "@dcl/one-ui/apps/sites/places/PlacesPage";
import type { PlaceCardData, PlaceCardLabels } from "@dcl/one-ui/apps/sites/places/cards/types";

import { QUERY_CLIENT_CONFIG } from "@/core/lib/query/defaults";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import type { StoryId } from "@/core/lib/telemetry/story-id";
import { track } from "@/core/lib/telemetry/track";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { useAuth } from "@/data/lib/auth/context";
import type { Destination } from "@/data/lib/catalyst/places/destinations";
import {
  browsePlacesQuery,
  CATEGORY_OPTIONS,
  effectiveSearch,
  liveFeedQuery,
  parseCategory,
  parseSection,
  type DiscoverCategory,
  type ExploreSection,
} from "@/data/lib/catalyst/places/discover";
import { openSignIn } from "@/features/components/auth/signin-store";
import { detectPlatform, type DetectedPlatform } from "@/features/download/options";
import { modalPlace, placeCard, placeJumpIn, type CreatorProfiles } from "@/features/places/cards";
import { useBrowsePlaces, useLiveRail, useMemberPlaces, usePlaceCreators } from "@/features/places/hooks";

import type { Route } from "./+types/places";

const STORY: StoryId = "misc/browse-places";
const SEARCH_DEBOUNCE_MS = 300;
/** Placeholder cards while a section loads: one row on the widest grid. */
const SKELETONS = 4;

const CATEGORY_LABELS: Record<DiscoverCategory, string> = {
  all: "Any Category",
  featured: "Featured",
  social: "Social",
  music: "Music",
  art: "Art",
  game: "Game",
  fashion: "Fashion",
  education: "Education",
  shop: "Shop",
  sports: "Sports",
  business: "Business",
};
const CATEGORIES = CATEGORY_OPTIONS.map((value) => ({ value, label: CATEGORY_LABELS[value] }));

const CARD_LABELS: PlaceCardLabels = { by: "By", jumpIn: "Jump in", featured: "Featured", live: "Live" };
const BAND_LABELS: ExploreBandLabels = {
  sections: { all: "Explore all", favourites: "Favourites", my: "My places" },
  sectionsGroup: "Places sections",
  search: "Search places",
  searchPlaceholder: "Search places",
  category: "Category",
  filters: "Filters",
  closeFilters: "Close",
  removeFilter: (category) => `Remove filter: ${category}`,
};
const MODAL_LABELS = {
  close: "Close",
  jumpIn: "Jump in",
  copyLink: "Copy link",
  copied: "Copied!",
  by: "By",
  whatToExpect: "What to expect",
};
const COPY = {
  live: "Live Now",
  empty: "No places match your filters.",
  emptyFavourites: "You haven't favourited any places yet.",
  emptyMine: "You haven't published any places yet.",
  signInFavourites: "Sign in and favourite places to find them here.",
  signInMine: "Sign in to see the places you created.",
  signIn: "Sign In",
  error: "Something went wrong loading places.",
  retry: "Retry",
  loading: "Loading places…",
  loadingMore: "Loading more places…",
  loadMore: "Load more places",
};

export function meta() {
  return [
    { title: "Places | Decentraland" },
    { name: "description", content: "Discover places in Decentraland, see who is online, and find your next virtual destination." },
  ];
}

export const handle = { agentMarkdown: "placesIndex" } satisfies AgentMarkdownHandle;

type View = { section: ExploreSection; category: DiscoverCategory; search: string };

function readView(params: URLSearchParams): View {
  return {
    section: parseSection(params.get("tab")),
    category: parseCategory(params.get("category")),
    search: effectiveSearch(params.get("search")),
  };
}

/** Search collapses Live Now; a category or a member section leaves it in place (source, 2026-10-13). */
function showsLive(view: View): boolean {
  return view.section !== "all" || !view.search;
}

/**
 * The first document: Explore all's first page is required (an unavailable catalogue answers 503 with
 * the page's error state), Live Now is optional. Member sections need the visitor's signature, so the
 * browser reads them.
 */
export async function loader({ request }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request);
  const view = readView(new URL(request.url).searchParams);
  const client = new QueryClient(QUERY_CLIENT_CONFIG);
  let unavailable = false;
  const reads = Promise.all([
    view.section === "all"
      ? client.fetchInfiniteQuery(browsePlacesQuery(view)).catch(() => {
          // A visitor who left aborted the read: that is not an outage.
          if (!request.signal.aborted) unavailable = true;
        })
      : undefined,
    showsLive(view) ? client.prefetchQuery(liveFeedQuery()) : undefined,
  ]);
  // The query functions read their own signal; cancelling the queries aborts it, and so the upstream reads.
  const cancel = () => void client.cancelQueries();
  if (request.signal.aborted) cancel();
  else request.signal.addEventListener("abort", cancel, { once: true });
  try {
    await reads;
  } finally {
    request.signal.removeEventListener("abort", cancel);
  }
  const dehydratedState = dehydrate(client);
  client.clear();
  return wrap(
    { sid, platform: detectPlatform(request.headers.get("user-agent")), dehydratedState, unavailable },
    { status: unavailable ? 503 : 200 },
  );
}

/** Jump In's target depends on the visitor's device. */
export function headers() {
  return { Vary: "User-Agent" };
}

/** Client navigations fetch through the browser cache; the server loader runs for the first document only. */
export async function clientLoader(): Promise<null> {
  return null;
}

/** The session id of the first document, kept for client navigations (browser only, set from an effect). */
let browserSid = "";

const subscribeHydration = () => () => {};

export default function Places({ loaderData }: Route.ComponentProps) {
  return (
    <HydrationBoundary state={loaderData?.dehydratedState}>
      <PlacesExplorer loaderSid={loaderData?.sid} loaderPlatform={loaderData?.platform} unavailable={loaderData?.unavailable ?? false} />
    </HydrationBoundary>
  );
}

type ExplorerProps = { loaderSid?: string; loaderPlatform?: DetectedPlatform; unavailable: boolean };

function PlacesExplorer({ loaderSid, loaderPlatform, unavailable }: ExplorerProps) {
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const view = readView(params);
  const rawSearch = params.get("search") ?? "";
  const hydrated = useSyncExternalStore(subscribeHydration, () => true, () => false);
  const [platform] = useState<DetectedPlatform>(
    () => loaderPlatform ?? (typeof navigator === "undefined" ? { kind: "unknown" } : detectPlatform(navigator.userAgent)),
  );
  useEffect(() => {
    if (loaderSid) browserSid = loaderSid;
  }, [loaderSid]);
  const sid = loaderSid ?? browserSid;

  const auth = useAuth();
  const identity = auth.isConnected ? auth.identity : null;

  // A document the server answered 503 shows the error at once for the view it was asked for; Retry, not
  // hydration, reads that feed again, and any other category or search reads normally.
  const viewKey = `${view.section}|${view.category}|${view.search}`;
  const [heldKey, setHeldKey] = useState(unavailable ? viewKey : null);
  const held = heldKey === viewKey;
  const browse = useBrowsePlaces({ category: view.category, search: view.search }, view.section === "all" && !held);
  const live = useLiveRail(showsLive(view));
  const member = useMemberPlaces(view.section, hydrated ? identity : null);

  const liveRail = showsLive(view) ? live.rail : [];
  // Under a category the grid is the complete filtered result, so Live Now takes nothing out of it.
  const exploreRows = useMemo(() => {
    if (view.category !== "all") return browse.rows;
    const shown = new Set(liveRail.map((place) => place.id));
    return browse.rows.filter((place) => !shown.has(place.id));
  }, [browse.rows, liveRail, view.category]);
  const sectionRows = view.section === "all" ? exploreRows : member.rows;
  const visible = useMemo(() => [...liveRail, ...sectionRows], [liveRail, sectionRows]);
  const creatorGroups = useMemo(
    () => [liveRail, ...(view.section === "all" ? browse.pageRows : [member.rows])],
    [liveRail, view.section, browse.pageRows, member.rows],
  );
  const profiles = usePlaceCreators(creatorGroups);

  const places = useMemo(() => new Map(visible.map((place) => [place.id, place])), [visible]);
  const toCards = (rows: readonly Destination[], creators: CreatorProfiles) => rows.map((place) => placeCard(place, platform, creators));
  const liveCards = toCards(liveRail, profiles);
  const sectionCards = toCards(sectionRows, profiles);

  const [modal, setModal] = useState<{ place: Destination; card: PlaceCardData } | null>(null);

  // Analytics: one list view per filter set, as the previous page reported it.
  const viewed = useRef<string | null>(null);
  const listKey = `${view.category}|${view.search}`;
  useEffect(() => {
    if (!sid || view.section !== "all" || !browse.data || viewed.current === listKey) return;
    viewed.current = listKey;
    track(
      "place_list_viewed",
      { count: browse.rows.length, category: view.category === "all" ? null : view.category, search: view.search || null },
      { sid, story: STORY },
    );
  }, [sid, listKey, view.section, view.category, view.search, browse.data, browse.rows.length]);

  // Search types into the field at once and reaches the URL (and the feed) once typing pauses.
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

  const setParam = (key: string, value: string | null) =>
    setParams(
      (previous) => {
        const next = new URLSearchParams(previous);
        if (value) next.set(key, value);
        else next.delete(key);
        return next;
      },
      { preventScrollReset: true },
    );

  // A tab switch can shrink the page under the toolbar; bring it back under the navbar when it moved away.
  const band = useRef<HTMLDivElement>(null);
  const switched = useRef(false);
  useEffect(() => {
    if (!switched.current || !band.current) return;
    switched.current = false;
    const clearance = window.innerWidth >= 1024 ? 100 : 72;
    const top = band.current.getBoundingClientRect().top;
    if (top >= 0 && top <= clearance + 40) return;
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    band.current.scrollIntoView?.({ behavior: reduce ? "auto" : "smooth", block: "start" });
  }, [view.section]);

  /** Cards are plain links: a plain click stays in the app; modified and middle clicks keep the browser's behavior. */
  type Opens = "busy-navigates" | "modal" | "page";
  const open = (card: PlaceCardData, event: MouseEvent<HTMLAnchorElement>, opens: Opens) => {
    if (sid) track("place_card_clicked", { place_id: card.id }, { sid, story: STORY });
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    const place = places.get(card.id);
    const inPlace = opens === "modal" || (opens === "busy-navigates" && (place?.user_count ?? 0) === 0);
    if (place && inPlace && platform.kind !== "mobile") setModal({ place, card });
    else void navigate(card.href);
  };
  // On desktop, as source: an empty Explore card opens JUMP IN in place and a busy one the place page;
  // Favourites and My places always open JUMP IN (source reads their rows as empty); Live Now cards always
  // open the place page. On a phone every card opens the place page.
  const onOpen = (card: PlaceCardData, event: MouseEvent<HTMLAnchorElement>) => open(card, event, "busy-navigates");
  const onOpenMember = (card: PlaceCardData, event: MouseEvent<HTMLAnchorElement>) => open(card, event, "modal");
  const onOpenLive = (card: PlaceCardData, event: MouseEvent<HTMLAnchorElement>) => open(card, event, "page");
  const jumpIn = (place: Destination | undefined, event: MouseEvent<HTMLAnchorElement>) => {
    const handler = place ? placeJumpIn(place, platform).onJumpIn : undefined;
    handler?.(event);
  };
  const onJumpIn = (card: PlaceCardData, event: MouseEvent<HTMLAnchorElement>) => jumpIn(places.get(card.id), event);

  const status = view.section === "all" ? browseStatus() : memberStatus();
  function browseStatus(): string {
    // The next page announces itself from the grid's sentinel.
    return browse.isPending ? COPY.loading : "";
  }
  function memberStatus(): string {
    return identity && member.isPending ? COPY.loading : "";
  }

  return (
    <SitesChrome active="explore">
      <PlacesPage
        title="Places"
        status={status}
        live={
          liveCards.length ? (
            <LiveNow
              title={COPY.live}
              cards={liveCards}
              cardLabels={CARD_LABELS}
              labels={{ previous: "Previous live places", next: "More live places", goTo: (n, title) => `Show ${title} (${n})` }}
              onOpen={onOpenLive}
              onJumpIn={onJumpIn}
            />
          ) : null
        }
      >
        <ExploreBand
          ref={band}
          section={view.section}
          onSection={(section) => {
            switched.current = true;
            setParam("tab", section === "all" ? null : section);
          }}
          search={searchInput}
          onSearch={setSearchInput}
          category={view.category}
          categories={CATEGORIES}
          onCategory={(value) => setParam("category", value === "all" ? null : value)}
          allCategory="all"
          labels={BAND_LABELS}
        >
          {view.section === "all" ? exploreResults() : memberResults()}
        </ExploreBand>
      </PlacesPage>
      {modal ? (
        <JumpInModal
          place={modalPlace(modal.place, modal.card, window.location.origin)}
          labels={MODAL_LABELS}
          onClose={() => setModal(null)}
          onJumpIn={(event) => jumpIn(modal.place, event)}
        />
      ) : null}
    </SitesChrome>
  );

  function exploreResults() {
    if (held) {
      return <PlacesMessage tone="alert" message={COPY.error} action={{ label: COPY.retry, onClick: () => setHeldKey(null) }} />;
    }
    if (browse.isError && !browse.data) {
      return <PlacesMessage tone="alert" message={COPY.error} action={{ label: COPY.retry, onClick: () => void browse.refetch() }} />;
    }
    if (browse.isPending) return <PlaceGrid cards={[]} skeletons={SKELETONS} labels={CARD_LABELS} label={BAND_LABELS.sections.all} />;
    // Judged before Live Now's deduplication: a first page Live Now holds entirely is not an empty result.
    if (!browse.rows.length && !browse.hasNextPage) return <PlacesMessage message={COPY.empty} />;
    return (
      <>
        <PlaceGrid cards={sectionCards} labels={CARD_LABELS} label={BAND_LABELS.sections.all} onOpen={onOpen} onJumpIn={onJumpIn} />
        {browse.isFetchNextPageError ? (
          <PlacesMessage tone="alert" message={COPY.error} action={{ label: COPY.retry, onClick: () => void browse.fetchNextPage() }} />
        ) : (
          <LoadMore
            hasMore={browse.hasNextPage}
            loading={browse.isFetchingNextPage}
            onLoadMore={() => void browse.fetchNextPage()}
            loadingLabel={COPY.loadingMore}
            loadMoreLabel={COPY.loadMore}
          />
        )}
      </>
    );
  }

  function memberResults() {
    const favourites = view.section === "favourites";
    const label = BAND_LABELS.sections[view.section];
    if (!hydrated) return <PlaceGrid cards={[]} skeletons={SKELETONS} labels={CARD_LABELS} label={label} />;
    if (!identity) {
      return (
        <PlacesMessage
          message={favourites ? COPY.signInFavourites : COPY.signInMine}
          action={{ label: COPY.signIn, onClick: () => openSignIn() }}
        />
      );
    }
    if (member.isError) {
      return <PlacesMessage tone="alert" message={COPY.error} action={{ label: COPY.retry, onClick: () => void member.refetch() }} />;
    }
    if (member.isPending) return <PlaceGrid cards={[]} skeletons={SKELETONS} labels={CARD_LABELS} label={label} />;
    if (!sectionCards.length) return <PlacesMessage message={favourites ? COPY.emptyFavourites : COPY.emptyMine} />;
    return <PlaceGrid cards={sectionCards} labels={CARD_LABELS} label={label} onOpen={onOpenMember} onJumpIn={onJumpIn} />;
  }
}

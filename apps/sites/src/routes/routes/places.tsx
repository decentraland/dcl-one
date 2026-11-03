import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { ShouldRevalidateFunctionArgs } from "react-router";
import { Link, useNavigate, useNavigation, useRevalidator, useSearchParams } from "react-router";

import ExploreChrome, { EXPLORE_TABS, PUBLIC_WORLD_TABS, type TabId } from "@ui/explorer/frames/ExploreChrome";
import PlaceCard from "@ui/components/PlaceCard";
import SearchField from "@ui/atoms/SearchField";
import FreshnessNotice from "@ui/components/FreshnessNotice";
import type { CopyStamp } from "@ui/data/copyAsOf";
import { olderCopy } from "@ui/data/copyAsOf";
import "@ui/explorer/pages/places.css";

import { mapPlace, type Place, type Category } from "@/data/lib/catalyst/places/index";
import { loadCategories } from "@/data/lib/catalyst/places/index.server";
import { fetchDestinations, hiddenFromBrowse, type DestinationPage } from "@/data/lib/catalyst/places/destinations";
import { useAuth } from "@/data/lib/auth/context";
import { openSignIn } from "@/features/components/auth/signin-store";
import { makeSearchCache } from "@/core/lib/router/client-cache";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/places";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "misc/browse-places";

const PLACES_LIMIT = 60;
type Section = "all" | "favourites" | "my";
const SECTIONS: { id: Section; label: string }[] = [{ id: "all", label: "Explore all" }, { id: "favourites", label: "Favourites" }, { id: "my", label: "My places" }];
const EMPTY_PAGE: DestinationPage = { data: [], total: 0, copy: null };

export function meta() {
  return [{ title: "Places | Decentraland" }, { name: "description", content: "Discover places in Decentraland, see who is online, and find your next virtual destination." }];
}

export const handle = { agentMarkdown: "placesIndex" } satisfies AgentMarkdownHandle;

const PLACES_TABS = EXPLORE_TABS.filter((tab) => PUBLIC_WORLD_TABS.has(tab.id));
const TAB_ROUTES: Partial<Record<TabId, string>> = {
  events: "/whats-on",
  places: "/places",
  communities: "/social",
  marketplace: "/shop",
  create: "/create",
};

const FALLBACK: PageDefaults = {
  variant: "live-signals",
};

export async function loader({ request }: Route.LoaderArgs) {
  const requestUrl = new URL(request.url);
  const category = requestUrl.searchParams.get("category")?.trim() ?? "";
  const search = requestUrl.searchParams.get("search")?.trim() ?? "";
  const tabParam = requestUrl.searchParams.get("tab");
  const section: Section = tabParam === "my" ? "my" : tabParam === "favourites" || tabParam === "favorites" ? "favourites" : "all";
  const rawOffset = Number.parseInt(requestUrl.searchParams.get("offset") ?? "0", 10);
  const offset = Number.isFinite(rawOffset) && rawOffset > 0 ? rawOffset : 0;

  const highlights = section === "all" && !category && !search && offset === 0;
  const [{ sid, wrap }, placesRes, categories, featuredRes] = await Promise.all([
    pageLoader(request,  FALLBACK),
    (section === "all" ? fetchDestinations({
      limit: PLACES_LIMIT,
      offset: offset || undefined,
      categories: category || undefined,
      search: search || undefined,
    }, { signal: request.signal }) : Promise.resolve(EMPTY_PAGE))
      .then((page) => ({ ...page, unavailable: false }))
      .catch(() => ({ ...EMPTY_PAGE, unavailable: true })),
    loadCategories().catch(() => [] as Category[]),
    highlights ? fetchDestinations({ limit: 100, only_highlighted: true }, { signal: request.signal }).catch(() => EMPTY_PAGE) : Promise.resolve(EMPTY_PAGE),
  ]);

  return wrap({
    sid,
    category,
    search,
    section,
    offset,
    places: placesRes.data,
    total: placesRes.total,
    received: placesRes.received,
    categories,
    live: highlights ? placesRes.data : [],
    featured: featuredRes.data,
    unavailable: placesRes.unavailable,
    layoutV2: false,
    copy: { stamp: olderCopy(placesRes.copy, featuredRes.copy), now: Date.now() },
  }, { status: placesRes.unavailable ? 503 : 200 });
}

type LoaderData = {
  sid: string;
  category: string;
  search: string;
  section: Section;
  offset: number;
  places: Place[];
  total: number;
  received?: number;
  categories: Category[];
  live: Place[];
  featured: Place[];
  unavailable: boolean;
  layoutV2: boolean;
  copy: CopyView;
};

/** The places copy's stamp, and the time the loader read it (one clock for SSR and hydration). */
type CopyView = { stamp: CopyStamp | null; now: number };

const cache = makeSearchCache<LoaderData>();

export async function clientLoader({
  request,
  serverLoader,
}: Route.ClientLoaderArgs) {
  if (isPersonal(new URL(request.url))) return serverLoader();
  const key = cache.key(request);
  const hit = cache.get(key);
  if (hit) return hit;
  const fresh = (await serverLoader()) as LoaderData;
  if (!fresh.unavailable) cache.set(key, fresh);
  return fresh;
}

function isPersonal(url: URL): boolean {
  return ["my", "favourites", "favorites"].includes(url.searchParams.get("tab") ?? "");
}

export function shouldRevalidate({ currentUrl, nextUrl, defaultShouldRevalidate }: ShouldRevalidateFunctionArgs) {
  if (currentUrl.pathname === nextUrl.pathname && currentUrl.search === nextUrl.search) {
    if (defaultShouldRevalidate) cache.delete(nextUrl.search);
    return defaultShouldRevalidate;
  }
  if (currentUrl.pathname === nextUrl.pathname && isPersonal(currentUrl) && isPersonal(nextUrl)) {
    const current = new URLSearchParams(currentUrl.search);
    const next = new URLSearchParams(nextUrl.search);
    current.delete("offset");
    next.delete("offset");
    if (current.toString() === next.toString()) return false;
  }
  return defaultShouldRevalidate;
}

export default function Places({ loaderData }: Route.ComponentProps) {
  const d = loaderData;
  const [params] = useSearchParams();
  const requestedOffset = Number.parseInt(params.get("offset") ?? "0", 10);
  const offset = d.section === "all" ? d.offset : Number.isFinite(requestedOffset) && requestedOffset > 0 ? requestedOffset : 0;

  return (
    <PlacesExplorer
      sid={d.sid}
      activeCategory={d.category}
      search={d.search}
      section={d.section}
      offset={offset}
      places={d.places}
      total={d.total}
      received={d.received}
      categories={d.categories}
      live={d.live}
      featured={d.featured}
      unavailable={d.unavailable}
      layoutV2={d.layoutV2}
      copy={d.copy}
    />
  );
}

type ExplorerProps = {
  sid: string;
  activeCategory: string;
  search: string;
  section: Section;
  offset: number;
  places: Place[];
  total: number;
  received?: number;
  categories: Category[];
  live: Place[];
  featured: Place[];
  unavailable: boolean;
  layoutV2: boolean;
  copy: CopyView;
};

const subscribeHydration = () => () => {};

type LoadedPages = { key: string; pages: Record<number, DestinationPage> };

function PlacesExplorer({
  sid,
  activeCategory,
  search,
  section,
  offset,
  places,
  total,
  received,
  categories,
  live,
  featured,
  unavailable,
  layoutV2,
  copy,
}: ExplorerProps) {
  const [, setSearchParams] = useSearchParams();
  const navigation = useNavigation();
  const navigate = useNavigate();
  const revalidator = useRevalidator();
  const auth = useAuth();
  const identity = auth.isConnected ? auth.identity : null;
  const personal = section !== "all";
  const hydrated = useSyncExternalStore(subscribeHydration, () => true, () => false);
  const personalAccount = personal ? identity?.signer.toLowerCase() ?? null : null;
  const previousAccount = useRef<string | null | undefined>(undefined);
  const accountChanged = previousAccount.current !== undefined && previousAccount.current !== null && previousAccount.current !== personalAccount;
  useEffect(() => {
    if (!hydrated || !personal) return;
    previousAccount.current = personalAccount;
    if (accountChanged) setSearchParams((previous) => {
      if (!previous.has("offset")) return previous;
      const next = new URLSearchParams(previous); next.delete("offset"); return next;
    }, { replace: true, preventScrollReset: true });
  }, [hydrated, personal, personalAccount, accountChanged, setSearchParams]);
  const key = JSON.stringify([personal ? personalAccount ?? "signed-out" : "public", section, activeCategory, search]);
  const personalKey = JSON.stringify([key, offset]);
  const [personalAttempt, setPersonalAttempt] = useState(0);
  const [personalRead, setPersonalRead] = useState<{ key: string; page?: DestinationPage; error?: string } | null>(null);
  const [personalPages, setPersonalPages] = useState<LoadedPages>({ key, pages: {} });
  const identityRef = useRef(identity);
  useEffect(() => { identityRef.current = identity; }, [identity]);
  useEffect(() => {
    const currentIdentity = identityRef.current;
    if (!hydrated || !personal || !currentIdentity || (accountChanged && offset > 0)) return;
    const controller = new AbortController();
    fetchDestinations({
      limit: PLACES_LIMIT, offset, categories: activeCategory || undefined, search: search || undefined,
      only_favorites: section === "favourites" || undefined, owner: section === "my" ? currentIdentity.signer : undefined,
    }, { identity: currentIdentity, signal: controller.signal }).then(
      (page) => {
        if (controller.signal.aborted) return;
        setPersonalRead({ key: personalKey, page });
        setPersonalPages((previous) => ({ key, pages: { ...(previous.key === key ? previous.pages : {}), [offset]: page } }));
      },
      (error: unknown) => { if (!controller.signal.aborted) setPersonalRead({ key: personalKey, error: error instanceof Error ? error.message : "Could not load your places." }); },
    );
    return () => controller.abort();
  }, [hydrated, personal, personalAccount, key, personalKey, section, activeCategory, search, offset, personalAttempt, accountChanged]);
  const personalResult = personalRead?.key === personalKey ? personalRead : null;
  const personalLoading = personal && Boolean(identity) && !personalResult;
  const loadedPages = personalPages.key === key ? Object.entries(personalPages.pages).filter(([start]) => Number(start) <= offset).sort(([a], [b]) => Number(a) - Number(b)) : [];
  const lastPage = loadedPages.at(-1);
  const page = personal ? personalResult?.page ?? lastPage?.[1] ?? EMPTY_PAGE : { data: places, total, copy: copy.stamp, received };
  const listError = personal ? personalResult?.error : unavailable ? "Places are temporarily unavailable. Please try again." : undefined;
  const highlights = !personal && !activeCategory && !search;

  const [publicPages, setPublicPages] = useState<LoadedPages>(() => ({ key, pages: { [offset]: { data: places, total, received, copy: copy.stamp } } }));
  useEffect(() => {
    if (!personal && !unavailable) setPublicPages((previous) => ({
      key,
      pages: { ...(previous.key === key && offset > 0 ? previous.pages : {}), [offset]: { data: places, total, received, copy: copy.stamp } },
    }));
  }, [personal, unavailable, key, offset, places, total, received, copy.stamp]);
  const publicRows = publicPages.key === key
    ? Object.entries(publicPages.pages).filter(([start]) => Number(start) <= offset).sort(([a], [b]) => Number(a) - Number(b)).flatMap(([, loaded]) => loaded.data)
    : places;
  const rows = [...new Map((personal ? loadedPages.flatMap(([, loaded]) => loaded.data) : publicRows).map((place) => [place.id, place])).values()];
  // Keep first-page rails stable while subsequent pages are appended.
  const [initialRails, setInitialRails] = useState(() => ({ key, live, featured }));
  useEffect(() => {
    if (offset === 0 && !unavailable) setInitialRails({ key, live, featured });
  }, [key, offset, unavailable, live, featured]);
  const rails = offset === 0 && !unavailable ? { live, featured }
    : initialRails.key === key ? initialRails : { live: [], featured: [] };

  useExposure(sid, rows.length, activeCategory, search);

  const liveNow = highlights ? rails.live.filter((place) => !hiddenFromBrowse(place) && (place.user_count ?? 0) > 0).sort((a, b) => (b.user_count ?? 0) - (a.user_count ?? 0)).slice(0, 4) : [];
  const liveIds = new Set(liveNow.map((place) => place.id));
  const featuredPlaces = highlights ? rails.featured.filter((place) => !hiddenFromBrowse(place) && !liveIds.has(place.id)) : [];
  const featuredIds = new Set(featuredPlaces.map((place) => place.id));
  const mainRows = rows.filter((place) => (personal || Boolean(search) || Boolean(activeCategory) || !hiddenFromBrowse(place)) && (!highlights || (!liveIds.has(place.id) && !featuredIds.has(place.id))));
  const nextOffset = (personal && lastPage ? Number(lastPage[0]) : offset) + (page.received ?? page.data.length);
  const canLoadMore = nextOffset < page.total && (page.received ?? page.data.length) === PLACES_LIMIT;
  const loadingMore = navigation.state === "loading" || personalLoading;

  function updateParam(paramKey: string, value: string) {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        if (value) next.set(paramKey, value);
        else next.delete(paramKey);
        if (paramKey !== "offset") next.delete("offset");
        return next;
      },
      { preventScrollReset: true },
    );
  }

  function onCardClick(placeId: string) {
    track("place_card_clicked", { place_id: placeId }, { sid, story: STORY });
  }

  return (
    <ExploreChrome
      active="places"
      tabs={PLACES_TABS}
      user={auth.address ? `${auth.address.slice(0, 6)}…${auth.address.slice(-4)}` : "Guest"}
      address={auth.address ?? undefined}
      isGuest={!auth.isConnected}
      onLobbyOpen={() => navigate("/play")}
      onTab={(tab) => { const route = TAB_ROUTES[tab]; if (route) navigate(route); }}
      onClose={() => navigate("/play")}
    >
      <div className="pl" data-places-layout={layoutV2 ? "v2" : "v1"}>
        <div className="pl__head">
          <h1 className="pl__title">Places</h1>
          <div className="pl__actions">
            <div className="pl__search">
              <SearchField
                placeholder="Search"
                value={search}
                onChange={(v: string) => updateParam("search", v)}
              />
            </div>
          </div>
        </div>

        <FreshnessNotice copy={page.copy} now={copy.now} />

        {liveNow.length > 0 && <section className="pl__highlight" aria-label="Live places"><h2 className="pl__highlighttitle"><span className="pl__livedot" />Live Now</h2><PlaceGrid places={liveNow} onCardClick={onCardClick} /></section>}
        {featuredPlaces.length > 0 && <section className="pl__highlight" aria-label="Featured places"><h2 className="pl__highlighttitle">Featured Places</h2><PlaceGrid places={featuredPlaces} onCardClick={onCardClick} /></section>}

        <div className="pl__sections" role="tablist" aria-label="Places sections">
          {SECTIONS.map((tab) => <CategoryPill key={tab.id} label={tab.label} active={section === tab.id} onSelect={() => updateParam("tab", tab.id === "all" ? "" : tab.id)} />)}
        </div>

        <div className="pl__cats" role="tablist" aria-label="Place categories">
          <CategoryPill
            label="All"
            active={activeCategory === ""}
            onSelect={() => updateParam("category", "")}
          />
          {categories.map((c) => (
            <CategoryPill
              key={c.name}
              label={c.i18n?.en ?? c.name}
              active={activeCategory === c.name}
              onSelect={() => updateParam("category", c.name)}
            />
          ))}
        </div>

        {personal && !hydrated ? <p role="status">Checking sign-in…</p>
          : personal && !identity ? <div className="pl__statemsg"><p>Sign in to see {section === "favourites" ? "your favourites" : "your places"}.</p><button type="button" className="pl__pill" onClick={() => openSignIn()}>Sign in</button></div>
          : <>
            {listError && <div className="pl__statemsg"><p role="alert">{listError}</p><button type="button" className="pl__pill" onClick={() => { if (personal) { setPersonalRead(null); setPersonalAttempt((previous) => previous + 1); } else void revalidator.revalidate(); }}>Retry</button></div>}
            {personalLoading && rows.length === 0 ? <p role="status">Loading your places…</p>
              : mainRows.length > 0 ? <PlaceGrid places={mainRows} onCardClick={onCardClick} />
              : !listError && (canLoadMore ? <p className="pl__empty">No visible places on this page. Load more to keep exploring.</p>
                : liveNow.length === 0 && featuredPlaces.length === 0 ? <p className="pl__empty">{section === "favourites" ? "No favourites yet. Save places from their detail pages to find them here." : section === "my" ? "No places of your own found." : "No places found in this view."}</p> : null)}
          </>}

        {canLoadMore && (!personal || (hydrated && identity)) && (
          <div className="pl__loadmore">
            <button
              type="button"
              className="pl__pill"
              disabled={loadingMore || Boolean(listError)}
              onClick={() => updateParam("offset", String(nextOffset))}
            >
              {loadingMore ? "Loading\u{2026}" : "Load more places"}
            </button>
          </div>
        )}
      </div>
    </ExploreChrome>
  );
}

function PlaceGrid({ places, onCardClick }: { places: Place[]; onCardClick: (id: string) => void }) {
  return <div className="pl__grid">{places.map((place, index) => {
    const card = mapPlace(place);
    return <Link className="pl__destinationlink" key={place.id} to={`/places/${encodeURIComponent(place.id)}`} onClick={() => onCardClick(place.id)} prefetch="intent">
      <PlaceCard priority={index < 2} presentational title={card.title} image={card.image} players={card.players} rating={card.rating} coords={card.coords} live={card.live ? 1 : undefined} featured={card.featured} creator={card.creator} />
    </Link>;
  })}</div>;
}

type CategoryPillProps = {
  label: string;
  active: boolean;
  onSelect: () => void;
};

function CategoryPill({ label, active, onSelect }: CategoryPillProps) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      className={"pl__pill" + (active ? " is-active" : "")}
      onClick={onSelect}
    >
      {label}
    </button>
  );
}

function useExposure(
  sid: string,
  count: number,
  category: string,
  search: string,
) {
  const lastKey = useRef<string | null>(null);
  useEffect(() => {
    const key = `${category}|${search}`;
    if (lastKey.current === key) return;
    lastKey.current = key;
    track(
      "place_list_viewed",
      { count, category: category || null, search: search || null },
      { sid, story: STORY },
    );
  }, [sid, count, category, search]);
}

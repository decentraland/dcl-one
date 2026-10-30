import { useEffect, useRef } from "react";

import PlaceCard from "@ui/components/PlaceCard";
import "@ui/explorer/pages/places.css";

import UpstreamUnavailable from "../../components/UpstreamUnavailable";
import { mapPlace, type Place } from "@/data/lib/catalyst/places/index";
import {
  track as defaultTrack,
  type TrackContext,
  type TrackFn,
} from "@/core/lib/telemetry/track";

const PLACES_HREF = "/places";

function placeJumpPath(placeId: string): string {
  return `/places/${encodeURIComponent(placeId)}?from=open-screen`;
}

type OpenScreenProps = {
  places: Place[] | null;
  trackCtx: TrackContext;
  track?: TrackFn;
  navigate?: (url: string) => void;
};

function defaultNavigate(url: string): void {
  if (typeof window !== "undefined") window.location.assign(url);
}

export default function OpenScreen({
  places,
  trackCtx,
  track = defaultTrack,
  navigate = defaultNavigate,
}: OpenScreenProps) {
  const shown = useRef(false);
  const opened = useRef(false);

  useEffect(() => {
    if (shown.current) return;
    shown.current = true;
    track("cl_open_screen_shown", {}, trackCtx);
  }, [track, trackCtx]);

  useEffect(() => {
    if (opened.current || places === null) return;
    opened.current = true;
    track("cl_explore_opened", { place_count: places.length }, trackCtx);
  }, [places, track, trackCtx]);

  if (places === null) {
    return (
      <UpstreamUnavailable
        title="Places are temporarily unavailable"
        message="We couldn't load the things-to-do list. Please try again in a moment."
        backHref={PLACES_HREF}
        backLabel="Open Places"
      />
    );
  }

  function onCardClick(e: React.MouseEvent, placeId: string) {
    e.preventDefault();
    track("place_card_clicked", { place_id: placeId }, trackCtx);
    track("cl_open_jumped_in", { place_id: placeId }, trackCtx);
    navigate(placeJumpPath(placeId));
  }

  return (
    <section style={SCREEN} aria-label="Things to do">
      <header style={HEAD}>
        <h1 style={TITLE}>Things to do</h1>
        <a
          href={PLACES_HREF}
          onClick={(e) => {
            e.preventDefault();
            navigate(PLACES_HREF);
          }}
          style={SUBLINK}
        >
          Browse all places &#x2192;
        </a>
      </header>
      {places.length > 0 ? (
        <div className="pl__grid">
          {places.map((place, i) => {
            const card = mapPlace(place);
            return (
              <a
                key={card.id}
                href={placeJumpPath(card.id)}
                onClick={(e) => onCardClick(e, card.id)}
                style={{ display: "block", textDecoration: "none", color: "inherit" }}
              >
                <PlaceCard
                  title={card.title}
                  image={card.image}
                  players={card.players}
                  rating={card.rating}
                  coords={card.coords}
                  live={card.live ? 1 : undefined}
                  featured={card.featured}
                  creator={card.creator}
                  hue={(i * 47) % 360}
                  presentational
                />
              </a>
            );
          })}
        </div>
      ) : (
        <p style={MUTED}>No places found.</p>
      )}
    </section>
  );
}

const SCREEN: React.CSSProperties = {
  maxWidth: 1080,
  margin: "0 auto",
  padding: "32px 24px 48px",
  color: "var(--text, #fff)",
};
const HEAD: React.CSSProperties = {
  display: "flex",
  alignItems: "baseline",
  justifyContent: "space-between",
  gap: 16,
  margin: "0 0 20px",
};
const TITLE: React.CSSProperties = { margin: 0, fontSize: 26, lineHeight: 1.2 };
const SUBLINK: React.CSSProperties = {
  color: "var(--ink-7, rgba(255,255,255,0.7))",
  fontSize: 14,
  textDecoration: "none",
};
const MUTED: React.CSSProperties = {
  color: "var(--ink-6, rgba(255,255,255,0.6))",
  fontSize: 14,
  margin: 0,
  padding: "8px 0",
};

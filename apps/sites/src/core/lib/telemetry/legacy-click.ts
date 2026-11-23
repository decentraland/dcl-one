import { useEffect, useState } from "react";

import { collectCampaignParams } from "./campaign-params";
import { legacyAnalyticsReady, track, whenLegacyAnalyticsReady, type TrackContext } from "./track";
import { trackedUserDropped } from "./tracked-user";

/** The legacy site's `useDeferredTrack` audit fields. */
export type LegacyAudit = { track_called_at: number; track_delivered_at: number; track_deferred: boolean };

/**
 * The pending events of one mounted component, as the legacy `useDeferredTrack` queue was: what has not left
 * when the component unmounts is dropped, so it never leaves later under another page or another account.
 */
export type LegacyTrackScope = {
  defer(fire: () => void): void;
  /** Mounted: deferrals are accepted. */
  open(): void;
  /** Unmounted: pending deferrals are dropped and new ones refused. */
  close(): void;
};

export function createLegacyTrackScope(): LegacyTrackScope {
  const pending = new Set<() => void>();
  let open = true;
  return {
    defer(fire) {
      if (!open) return;
      let done = false;
      let cancel: (() => void) | null = null;
      cancel = whenLegacyAnalyticsReady(() => {
        done = true;
        if (cancel) pending.delete(cancel);
        fire();
      });
      if (!done) pending.add(cancel);
    },
    open() {
      open = true;
    },
    close() {
      open = false;
      for (const cancel of pending) cancel();
      pending.clear();
    },
  };
}

/** A scope for the calling component's lifetime (a Strict Mode remount reopens it). */
export function useLegacyTrackScope(): LegacyTrackScope {
  const [scope] = useState(createLegacyTrackScope);
  useEffect(() => {
    scope.open();
    return () => scope.close();
  }, [scope]);
  return scope;
}

/**
 * Who acted: the session id, the server-verified wallet when the visitor is signed in, and the emitting
 * component's scope (`useLegacyTrackScope`).
 */
export type LegacySession = { sid: string; userId?: string | null; scope: LegacyTrackScope };

/**
 * The legacy `useDeferredTrack` transport. `track_deferred` is true when the call came before the document's
 * analytics was ready (`legacyAnalyticsReady`). A conversion leaves at once, as the legacy beacon did; anything
 * else waits for readiness inside the emitting component's scope, so a bot or an exempt session never sends it
 * and an unmount drops it. `send` builds the event, so the contract harvests each caller's literal name and shape.
 */
export function sendLegacyDeferred(
  send: (audit: LegacyAudit, ctx: TrackContext) => void,
  { sid, userId, scope }: LegacySession,
  { conversion = false }: { conversion?: boolean } = {},
): void {
  const calledAt = Date.now();
  const deferred = !legacyAnalyticsReady();
  const fire = () => {
    // A sign-out in progress leaves the account out before root's data has reloaded without it.
    const user = userId && !trackedUserDropped() ? userId : undefined;
    send(
      // A conversion leaves at once, so its delivery is its call, as the legacy beacon reported it.
      { track_called_at: calledAt, track_delivered_at: conversion ? calledAt : Date.now(), track_deferred: deferred },
      { sid, ...(user ? { userId: user } : {}), legacy: true, ...(conversion ? { conversion: true } : {}) },
    );
  };
  if (conversion) fire();
  else scope.defer(fire);
}

/** The legacy `data-place` of a non-download control (decentraland/sites `SectionViewedTrack` values). */
export type LegacyClickPlace =
  | "Landing Catch The Vibe"
  | "Landing Explore"
  | "Landing Weekly Rituals"
  | "Creators Hero"
  | "Creators Why"
  | "Creators Create"
  | "Creators Live Scenes"
  | "Creators Connect"
  | "Creators Learn"
  | "Creators Blog"
  | "Creators Faqs"
  | "Referrer Invite First Hero"
  | "Referrer Invite Second Hero";

/** The control's other `data-*` attributes; an empty one was never sent. */
export type LegacyClick = { place: LegacyClickPlace; section?: string; card?: string; tab?: string; title?: string };

/**
 * The legacy `useTrackClick` `Click` of a control that downloads nothing: the URL's `utm_*`, then the control's
 * attributes (its `data-event` was `Click`, which the legacy payload dropped), then the audit fields.
 */
export function trackLegacyClick(click: LegacyClick, session: LegacySession): void {
  try {
    const attributes = Object.fromEntries(
      Object.entries(click).filter(([, value]) => value !== undefined && value !== ""),
    ) as LegacyClick;
    const base = { ...collectCampaignParams(), ...attributes };
    sendLegacyDeferred((audit, ctx) => track("Click", { ...base, ...audit }, ctx), session);
  } catch {
    // Telemetry never breaks a click.
  }
}

/** The /create sections that sent `Section Viewed` (the only page that did). */
export type LegacySectionViewed =
  | "Creators Why"
  | "Creators Create"
  | "Creators Live Scenes"
  | "Creators Connect"
  | "Creators Learn"
  | "Creators Blog"
  | "Creators Faqs";

/** decentraland-ui2's `useMobileMediaQuery` breakpoint, which the legacy `mobile` flag read. */
const LEGACY_MOBILE_QUERY = "(max-width: 767.95px)";

/** The legacy `Section Viewed`: the deferred transport, no `utm_*`. */
export function trackSectionViewed(section: LegacySectionViewed, session: LegacySession): void {
  try {
    const mobile = typeof window.matchMedia === "function" && window.matchMedia(LEGACY_MOBILE_QUERY).matches;
    sendLegacyDeferred(
      (audit, ctx) => track("Section Viewed", { section_viewed: section, mobile, ...audit }, ctx),
      session,
    );
  } catch {
    // Telemetry never breaks a page.
  }
}

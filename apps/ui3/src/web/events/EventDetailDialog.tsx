// from: sites@17b3535a:src/components/events/EventDetailModal (EventDetailModal, EventDetailModalHero,
// EventDetailModalContent), src/components/events/DetailModal/DetailModalCreator.tsx, src/hooks/useCreatorProfile.ts.
import { useEffect, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { JumpInCTA } from "@dcl/one-ui/components";

import foundationLogo from "../places/foundation-logo.svg?url";
import { linkifyText } from "./linkify";
import { sameOriginContentUrl } from "../../data/catalyst/profile";
import { fetchCommunityProfiles } from "../../data/hooks/useCommunityProfiles";
import { getAvatarBackgroundColor, getDisplayName } from "../../shop/discovery/lib/avatarColor";
import styles from "./EventDetailDialog.module.css";

export type EventDialogData = {
  id: string;
  name: string;
  description: string | null;
  image: string | null;
  x: number;
  y: number;
  creatorAddress?: string;
  creatorName?: string;
  startAt: string | null;
  finishAt: string | null;
  recurrent: boolean;
  recurrentFrequency: string | null;
  recurrentInterval: number | null;
  /** The weekdays of a weekly series, 0 = Sunday; undefined when the event has no weekday mask. */
  recurrentByDay?: number[];
  live: boolean;
  realm?: string;
  isWorld: boolean;
  placeName: string | null;
};

export type EventDetailDialogProps = {
  event: EventDialogData;
  signedIn: boolean;
  reminded?: boolean;
  remindPending?: boolean;
  onToggleRemind?: () => void;
  jumpHref?: string;
  onJumpIn?: () => void;
  shareUrl: string;
  onClose: () => void;
};

const FOUNDATION = "Decentraland Foundation";
const FOUNDATION_BACKGROUND = "#9d76e3";
const LOCALE = "en";

const noSubscribe = () => () => {};
const useHydrated = () => useSyncExternalStore(noSubscribe, () => true, () => false);

const TIME: Intl.DateTimeFormatOptions = { hour: "numeric", minute: "2-digit", hour12: true };
const DATE: Intl.DateTimeFormatOptions = { weekday: "short", month: "short", day: "numeric" };
const format = (at: string | number, options: Intl.DateTimeFormatOptions, timeZone?: string) => new Intl.DateTimeFormat(LOCALE, { ...options, timeZone }).format(new Date(at));
const formatLocalTime = (iso: string) => format(iso, TIME);
const formatLocalDate = (iso: string) => format(iso, DATE);
const utcTime = (iso: string) => format(iso, TIME, "UTC");
const utcDate = (iso: string) => format(iso, DATE, "UTC");
const SUNDAY_EPOCH_MS = Date.UTC(1970, 0, 4);
const weekday = (day: number, style: "short" | "long") => format(SUNDAY_EPOCH_MS + day * 86_400_000, { weekday: style }, "UTC");
const sortedDays = (days: number[]) => [...new Set(days)].filter((d) => d >= 0 && d <= 6).sort((a, b) => a - b);

function normalizeRecurrence(frequency: string | null, interval: number | null): { frequency: string | null; count: number } {
  const count = interval && interval > 1 ? interval : 1;
  if (frequency === "DAILY" && count > 1) {
    if (count % 365 === 0) return { frequency: "YEARLY", count: count / 365 };
    if (count % 7 === 0) return { frequency: "WEEKLY", count: count / 7 };
  }
  return { frequency, count };
}

const UNITS: Record<string, [noun: string, adverb: string]> = { DAILY: ["day", "daily"], WEEKLY: ["week", "weekly"], MONTHLY: ["month", "monthly"], YEARLY: ["year", "yearly"] };

/** The recurrence as the hero subtitle says it ("Every Monday", long weekdays) or as the schedule line does ("Repeats weekly", short weekdays). */
function recurrenceText(e: EventDialogData, hero: boolean): string | null {
  const { frequency, count } = normalizeRecurrence(e.recurrentFrequency, e.recurrentInterval);
  const byDay = e.recurrentByDay;
  if ((frequency === "WEEKLY" || frequency === "DAILY") && byDay && byDay.length > 0 && byDay.length < 7) {
    const days = sortedDays(byDay).map((d) => weekday(d, hero ? "long" : "short")).join(", ");
    return count > 1 ? `Every ${count} weeks on ${days}` : `${hero ? "Every" : "Repeats"} ${days}`;
  }
  const unit = frequency && UNITS[frequency];
  if (!unit) return null;
  if (count > 1) return `Every ${count} ${unit[0]}s`;
  if (!hero) return `Repeats ${unit[1]}`;
  return frequency === "WEEKLY" ? `Every ${format(e.startAt!, { weekday: "long" })}` : `Every ${unit[0]}`;
}

function heroSubtitle(e: EventDialogData): string | null {
  if (!e.startAt || Number.isNaN(new Date(e.startAt).getTime())) return null;
  const time = formatLocalTime(e.startAt);
  if (!e.recurrent) return `${formatLocalDate(e.startAt)} - ${time}`;
  const recurrence = recurrenceText(e, true);
  return recurrence ? `${recurrence} - ${time}` : null;
}

/** The whole local days between the local and the UTC calendar date of `iso`: -1, 0 or 1. */
function utcDayDelta(iso: string): -1 | 0 | 1 {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return 0;
  const local = d.getFullYear() * 10000 + (d.getMonth() + 1) * 100 + d.getDate();
  const utc = d.getUTCFullYear() * 10000 + (d.getUTCMonth() + 1) * 100 + d.getUTCDate();
  return utc > local ? 1 : utc < local ? -1 : 0;
}

/** The time in UTC, which the reference's tooltip shows and names its trigger with. */
function utcTooltip(startAt: string, finishAt: string | null): string {
  if (!finishAt) {
    const delta = utcDayDelta(startAt);
    return `${utcTime(startAt)} UTC${delta === 1 ? " (+1 day)" : delta === -1 ? " (-1 day)" : ""}`;
  }
  if (utcDayDelta(startAt) === 0 && utcDayDelta(finishAt) === 0) return `${utcTime(startAt)} – ${utcTime(finishAt)} UTC`;
  return `${utcDate(startAt)}, ${utcTime(startAt)} – ${utcDate(finishAt)}, ${utcTime(finishAt)} UTC`;
}

function scheduleText(e: EventDialogData): string {
  if (!e.startAt) return "";
  const range = `${formatLocalDate(e.startAt)} · ${formatLocalTime(e.startAt)}${e.finishAt ? ` – ${formatLocalTime(e.finishAt)}` : ""}`;
  return e.recurrent ? `Starting ${range}` : range;
}

const calendarStamp = (iso: string) => new Date(iso).toISOString().replace(/[-:]/g, "").replace(/\.\d{3}/, "");

function calendarUrl(e: EventDialogData, jumpHref: string | undefined): string | null {
  if (!e.startAt) return null;
  const start = calendarStamp(e.startAt);
  const params = new URLSearchParams({
    action: "TEMPLATE",
    text: e.name,
    dates: `${start}/${e.finishAt ? calendarStamp(e.finishAt) : start}`,
    details: `${e.description || ""}\n\n${jumpHref ?? ""}`,
    location: e.realm ? `Decentraland World (${e.realm})` : `Decentraland In-World (${e.x},${e.y})`,
  });
  const { frequency, count } = normalizeRecurrence(e.recurrentFrequency, e.recurrentInterval);
  if (e.recurrent && frequency) params.set("recur", `RRULE:FREQ=${frequency}${count > 1 ? `;INTERVAL=${count}` : ""}`);
  return `https://calendar.google.com/calendar/render?${params.toString()}`;
}

type Creator = { name?: string; face?: string; background: string; address?: string; foundation: boolean };

function useCreator(address: string | undefined, userName: string | undefined): Creator {
  const foundation = userName?.trim().toLowerCase() === FOUNDATION.toLowerCase();
  const profiles = useQuery({
    queryKey: ["event-creator-profile", address],
    queryFn: ({ signal }) => fetchCommunityProfiles([address as string], { signal }),
    enabled: Boolean(address) && !foundation,
    retry: false,
  }).data;
  if (foundation) return { name: FOUNDATION, face: foundationLogo, background: FOUNDATION_BACKGROUND, address, foundation };
  const key = address?.toLowerCase();
  const profile = key && profiles ? Object.entries(profiles).find(([a]) => a.toLowerCase() === key)?.[1] : undefined;
  const profileName = profile?.name || undefined;
  const name = profileName || userName || (address ? `${address.slice(0, 6)}…${address.slice(-4)}` : undefined);
  const background = getAvatarBackgroundColor(profileName
    ? getDisplayName(profileName, profile?.claimed, address)
    : name ? getDisplayName(name, false, address) : "");
  const face = profile?.picture ? sameOriginContentUrl(profile.picture) : undefined;
  return { name, face, background, address, foundation };
}

function Icon({ d, className }: { d: string; className?: string }) {
  return <svg className={`${styles.icon} ${className ?? ""}`} focusable="false" aria-hidden="true" viewBox="0 0 24 24" data-testid={ICON_NAMES.get(d)}><path d={d} />{d === PIN && <circle cx="12" cy="9" r="2.5" />}</svg>;
}

const CLOSE = "M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";
const BACK = "M17.77 3.77 16 2 6 12l10 10 1.77-1.77L9.54 12z";
const COPY = "M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2m0 16H8V7h11z";
const CALENDAR_MONTH = "M19 4h-1V2h-2v2H8V2H6v2H5c-1.11 0-1.99.9-1.99 2L3 20c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2m0 16H5V10h14zM9 14H7v-2h2zm4 0h-2v-2h2zm4 0h-2v-2h2zm-8 4H7v-2h2zm4 0h-2v-2h2zm4 0h-2v-2h2z";
const CALENDAR_TODAY = "M20 3h-1V1h-2v2H7V1H5v2H4c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2m0 18H4V8h16z";
const PIN = "M12 2C8.13 2 5 5.13 5 9c0 5.25 7 13 7 13s7-7.75 7-13c0-3.87-3.13-7-7-7M7 9c0-2.76 2.24-5 5-5s5 2.24 5 5c0 2.88-2.88 7.19-5 9.88C9.92 16.21 7 11.85 7 9";
const GLOBE = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2m-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39";
/** MUI icons carry their name as data-testid; the reference's icons are MUI's. */
const ICON_NAMES = new Map([
  [CLOSE, "CloseIcon"], [BACK, "ArrowBackIosNewIcon"], [COPY, "ContentCopyIcon"], [CALENDAR_MONTH, "CalendarMonthIcon"],
  [CALENDAR_TODAY, "CalendarTodayIcon"], [PIN, "LocationOnOutlinedIcon"], [GLOBE, "PublicRoundedIcon"],
]);
const BELL_BODY = "M5.4167 9.44547C5.44685 7.63928 6.21335 5.91687 7.55087 4.64979C8.88839 3.38271 10.6897 2.6725 12.5663 2.67236C14.4429 2.67223 16.2444 3.38217 17.5821 4.64907C18.9198 5.91596 19.6866 7.63825 19.717 9.44444L19.7173 12.2663L21.4284 15.9735C21.4888 16.1045 21.5143 16.2479 21.5025 16.3907C21.4907 16.5335 21.4421 16.6712 21.361 16.7914C21.2799 16.9115 21.169 17.0103 21.0382 17.0787C20.9074 17.1471 20.7609 17.183 20.6121 17.1831L15.3891 17.1835L15.3919 17.1891L9.73956 17.1894L9.74313 17.1835L4.52097 17.1824C4.37215 17.1824 4.2257 17.1465 4.0949 17.0782C3.96411 17.0099 3.85311 16.9112 3.77197 16.7911C3.69084 16.671 3.64213 16.5333 3.63028 16.3905C3.61843 16.2477 3.6438 16.1043 3.7041 15.9733L5.41606 12.2689L5.41581 9.44696L5.4167 9.44547ZM7.20443 9.44545L7.20361 12.4516C7.20351 12.5717 7.17727 12.6905 7.1266 12.8002L5.89689 15.4632L19.2368 15.4629L18.006 12.7997C17.9557 12.69 17.9298 12.5713 17.93 12.4514L17.9284 9.44595C17.9026 8.09324 17.3262 6.80431 16.3233 5.85649C15.3203 4.90867 13.971 4.37771 12.5655 4.37784C11.16 4.37797 9.81067 4.90918 8.80791 5.85719C7.80515 6.80519 7.22904 8.09422 7.20354 9.44694L7.20443 9.44545Z";
const BELL_CLAPPER = "M9.48535 18.7616C9.59625 19.475 9.96951 20.1265 10.5372 20.5974C11.1049 21.0683 11.8294 21.3274 12.5788 21.3275C13.3282 21.3277 14.0528 21.0689 14.6207 20.5982C15.1886 20.1276 15.5622 19.4763 15.6734 18.7629L9.48535 18.7616Z";
const BELL_ACTIVE = "M15.6737 18.7625C15.5625 19.4758 15.1888 20.1277 14.621 20.5984C14.0531 21.069 13.3282 21.328 12.579 21.3279C11.8296 21.3277 11.1047 21.0683 10.537 20.5974C9.96927 20.1265 9.59613 19.4749 9.48522 18.7615L15.6737 18.7625ZM12.5663 2.67261C14.4428 2.67251 16.2442 3.38232 17.5819 4.64917C17.9225 4.97176 18.2253 5.32444 18.4891 5.69995L16.4413 7.74976L16.1805 7.48901C15.0869 6.3954 13.3142 6.39538 12.2206 7.48901C11.1271 8.58265 11.127 10.3554 12.2206 11.449L14.4608 13.6892C14.9859 14.2141 15.6988 14.5095 16.4413 14.5095C17.1837 14.5093 17.8958 14.2142 18.4208 13.6892L19.7567 12.3523L21.4286 15.9734C21.4889 16.1042 21.5135 16.2477 21.5018 16.3904C21.4901 16.5332 21.4423 16.6716 21.3612 16.7917C21.2801 16.9119 21.1687 17.0105 21.038 17.0789C20.9073 17.1472 20.7609 17.1832 20.6122 17.1833H15.3885L15.3915 17.1892H9.73913L9.74304 17.1833L4.52038 17.1824C4.37169 17.1823 4.22528 17.1462 4.0946 17.0779C3.96389 17.0095 3.85245 16.9108 3.77136 16.7908C3.69038 16.6708 3.64159 16.533 3.62975 16.3904C3.61793 16.2476 3.6437 16.1043 3.70397 15.9734L5.41589 12.2693V9.44702L5.41686 9.44507C5.44712 7.63911 6.21335 5.91712 7.55065 4.65015C8.88817 3.38306 10.6897 2.67274 12.5663 2.67261Z";
const BELL_CHECK = "M20.0042 6.3392C20.4417 5.90174 21.1508 5.90174 21.5882 6.3392C22.0257 6.77666 22.0257 7.48574 21.5882 7.9232L17.1075 12.4039C16.8974 12.614 16.6126 12.7321 16.3155 12.7321C16.0184 12.7321 15.7336 12.614 15.5235 12.4039L13.2832 10.1635C12.8457 9.72609 12.8457 9.01701 13.2832 8.57955C13.7206 8.14209 14.4297 8.14209 14.8672 8.57955L16.3155 10.0279L20.0042 6.3392Z";

function RemindIcon({ active }: { active: boolean }) {
  return (
    <span className={styles.bell}>
      <svg className={styles.bellLayer} viewBox="0 0 25 25" fill="none" aria-hidden="true" style={{ opacity: active ? 0 : 1 }}>
        <path d={BELL_BODY} fill="currentColor" />
        <path d={BELL_CLAPPER} fill="currentColor" />
      </svg>
      <svg className={`${styles.bellLayer} ${styles.bellActive}`} viewBox="0 0 25 25" aria-hidden="true" style={{ opacity: active ? 1 : 0 }}>
        <path d={BELL_ACTIVE} />
        <path className={active ? styles.check : `${styles.check} ${styles.checkHidden}`} d={BELL_CHECK} />
      </svg>
    </span>
  );
}

/** Jump In: the library's JumpInCTA, sized as the reference's decentraland-ui2 Button (MUI medium or large). */
function JumpIn({ href, onClick, size }: { href?: string; onClick?: () => void; size: "medium" | "large" }) {
  const className = `${styles.jumpIn} ${size === "large" ? styles.jumpInLarge : styles.jumpInMedium}`;
  return href
    ? <JumpInCTA size="md" label="Jump In" className={className} href={href} onClick={onClick} />
    : <JumpInCTA size="md" label="Jump In" className={className} onClick={onClick} />;
}

function CreatorRow({ creator }: { creator: Creator }) {
  if (!creator.face && !creator.name) return null;
  const content: ReactNode = (
    <>
      {creator.face
        ? <img className={styles.avatar} src={creator.face} alt={creator.name ?? ""} style={{ backgroundColor: creator.background }} />
        : <div className={styles.avatar} style={{ backgroundColor: creator.background }} />}
      <p className={styles.creatorName}>By <span className={styles.creatorHighlight}>{creator.name}</span></p>
    </>
  );
  if (!creator.address || creator.foundation) return <div className={styles.creator}>{content}</div>;
  return (
    <a className={styles.creatorButton} href={`/profile/${creator.address}`} aria-label={creator.name ?? creator.address}>{content}</a>
  );
}

const FOCUSABLE = "a[href], button:not([disabled]), [tabindex]:not([tabindex='-1'])";

export default function EventDetailDialog({ event, signedIn, reminded = false, remindPending = false, onToggleRemind, jumpHref, onJumpIn, shareUrl, onClose }: EventDetailDialogProps) {
  const hydrated = useHydrated();
  const creator = useCreator(event.creatorAddress, event.creatorName);
  const paper = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    const { body } = document;
    const previous = body.style.overflow;
    body.style.overflow = "hidden";
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    paper.current?.focus({ preventScroll: true });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key !== "Tab" || !paper.current) return;
      const focusable = [...paper.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
      const first = focusable[0], last = focusable[focusable.length - 1];
      if (!first || !last) { e.preventDefault(); return; }
      const at = document.activeElement;
      const outside = !paper.current.contains(at) || at === paper.current;
      if (e.shiftKey && (at === first || outside)) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && (at === last || outside)) { e.preventDefault(); first.focus(); }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      body.style.overflow = previous;
      document.removeEventListener("keydown", onKey);
      opener?.focus({ preventScroll: true });
    };
  }, [onClose]);
  useEffect(() => {
    if (!copied) return undefined;
    const t = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(t);
  }, [copied]);

  const future = !event.live;
  const remindPrimary = future && signedIn;
  const calendarPrimary = future && !signedIn && Boolean(event.startAt);
  const remindSecondary = future && !signedIn;
  const calendarSecondary = !calendarPrimary && Boolean(event.startAt);
  const subtitle = hydrated ? heroSubtitle(event) : null;
  const schedule = hydrated ? scheduleText(event) : "";
  const utc = hydrated && event.startAt ? utcTooltip(event.startAt, event.finishAt) : undefined;
  const recurrence = event.recurrent ? recurrenceText(event, false) : null;
  const coords = event.placeName ? `${event.placeName} (${event.x},${event.y})` : `(${event.x},${event.y})`;
  const addToCalendar = () => {
    const url = calendarUrl(event, jumpHref);
    if (url) window.open(url, "_blank", "noopener,noreferrer");
  };
  const jump = () => {
    onJumpIn?.();
    if (jumpHref) window.location.assign(jumpHref);
  };
  const copyLink = () => {
    navigator.clipboard?.writeText(shareUrl).then(() => setCopied(true), () => undefined);
  };

  return (
    <div className={styles.root} role="presentation">
      <div className={styles.backdrop} aria-hidden="true" />
      <div className={styles.container} role="presentation" onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
        <div ref={paper} className={styles.paper} role="dialog" aria-modal="true" aria-labelledby="event-detail-title" tabIndex={-1}>
          <div className={styles.hero}>
            {event.image && <img className={styles.heroImage} src={event.image} alt={event.name} />}
            <div className={styles.heroOverlay} />
            <button type="button" className={styles.close} onClick={onClose} aria-label="Close">
              <Icon d={CLOSE} className={styles.closeIcon} />
              <Icon d={BACK} className={styles.backIcon} />
            </button>
            <div className={styles.heroContent}>
              {event.live
                ? <span className={styles.liveBadge}>Live</span>
                : subtitle && <div className={styles.timeTrigger}><p className={styles.subtitle}><time dateTime={event.startAt ?? undefined} title={utc}>{subtitle}</time></p></div>}
              <p className={styles.title} id="event-detail-title">{event.name}</p>
              <div className={styles.creatorLocation}>
                <CreatorRow creator={creator} />
                {event.isWorld
                  ? event.realm && (
                    <button type="button" className={styles.location} onClick={jump} aria-label={`Jump In – ${event.realm}`}>
                      <Icon d={GLOBE} className={styles.locationIcon} />
                      <span className={styles.locationText}>{event.realm}</span>
                    </button>
                  )
                  : (
                    <button type="button" className={styles.location} onClick={jump} aria-label={`Jump In – ${coords}`}>
                      <Icon d={PIN} className={styles.locationIcon} />
                      <span className={styles.locationText}>{coords}</span>
                    </button>
                  )}
              </div>
              <div className={styles.actions}>
                {event.live && <JumpIn href={jumpHref} onClick={onJumpIn} size="medium" />}
                {remindPrimary && (
                  <button type="button" className={styles.primary} onClick={onToggleRemind} disabled={remindPending} aria-label="Remind me" aria-pressed={reminded}>
                    <RemindIcon active={reminded} />
                    Remind me
                  </button>
                )}
                {calendarPrimary && (
                  <button type="button" className={styles.primary} onClick={addToCalendar} aria-label="Add to calendar">
                    <Icon d={CALENDAR_MONTH} className={styles.smallIcon} />
                    Add to calendar
                  </button>
                )}
                {remindSecondary && (
                  <button type="button" className={styles.secondary} onClick={onToggleRemind} disabled={remindPending} aria-label="Remind me">
                    <RemindIcon active={reminded} />
                  </button>
                )}
                {calendarSecondary && (
                  <button type="button" className={styles.secondary} onClick={addToCalendar} aria-label="Add to calendar">
                    <Icon d={CALENDAR_MONTH} className={styles.smallIcon} />
                  </button>
                )}
                <button type="button" className={styles.copy} onClick={copyLink} aria-label="Copy link">
                  <Icon d={COPY} className={styles.copyIcon} />
                  {copied && <span className={styles.copied} role="status">Copied!</span>}
                </button>
              </div>
            </div>
          </div>
          <div className={styles.content}>
            {event.description && (
              <>
                <p className={styles.label}>What to expect</p>
                <p className={styles.description}>{linkifyText(event.description)}</p>
              </>
            )}
            {event.description && event.startAt && <div className={styles.divider} />}
            {event.startAt && (
              <>
                <p className={styles.label}>Schedule</p>
                <div className={styles.scheduleRow}>
                  <div>
                    <div className={styles.timeTrigger}><p className={styles.scheduleText}><time dateTime={event.startAt} title={utc}>{schedule}</time></p></div>
                    {recurrence && <p className={styles.recurrence}>{recurrence}</p>}
                  </div>
                  <button type="button" className={styles.scheduleButton} onClick={addToCalendar} aria-label="Add to calendar">
                    <Icon d={CALENDAR_TODAY} className={styles.scheduleIcon} />
                  </button>
                </div>
              </>
            )}
            <div className={styles.bottomJump}>
              <JumpIn href={jumpHref} onClick={onJumpIn} size="large" />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

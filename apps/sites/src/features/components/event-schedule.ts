import { eventText, type EventsLocale, type EventCardTime } from "@dcl/one-ui";

import { eventOccurrence, type Event } from "@/data/lib/catalyst/places/events";

export type Zone = string | undefined;

export function formatTime(iso: string, timeZone: Zone, locale: EventsLocale = "en"): string {
  return new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit", hour12: locale === "en" ? true : undefined, timeZone }).format(
    new Date(iso),
  );
}

export function formatDate(iso: string, timeZone: Zone, locale: EventsLocale): string {
  return new Intl.DateTimeFormat(locale, { weekday: "short", month: "short", day: "numeric", timeZone }).format(
    new Date(iso),
  );
}

const FREQUENCY = {
  HOURLY: ["Every hour", "Every {count} hours"],
  DAILY: ["Repeats daily", "Every {count} days"],
  WEEKLY: ["Repeats weekly", "Every {count} weeks"],
  MONTHLY: ["Repeats monthly", "Every {count} months"],
  YEARLY: ["Repeats yearly", "Every {count} years"],
} as const;

type Recurrence = Pick<Event, "recurrent" | "recurrent_frequency" | "recurrent_interval">;
type DetailSchedule = Parameters<typeof eventOccurrence>[0] & Recurrence;

export function recurrenceLabel(
  event: Recurrence,
  locale: EventsLocale = "en",
): string | undefined {
  if (!event.recurrent || !event.recurrent_frequency) return undefined;
  if (!Object.hasOwn(FREQUENCY, event.recurrent_frequency)) return undefined;
  const labels = FREQUENCY[event.recurrent_frequency as keyof typeof FREQUENCY];
  const interval = event.recurrent_interval ?? 1;
  return eventText(locale, interval > 1 ? labels[1] : labels[0], { count: interval });
}

/** The hero line: "Wed, Nov 11 - 5:00 PM", or "Every Wednesday - 5:00 PM" for a weekly event. */
export function heroWhen(event: DetailSchedule, timeZone: Zone, locale: EventsLocale = "en"): EventCardTime | undefined {
  const { start } = eventOccurrence(event);
  if (!start || Number.isNaN(Date.parse(start))) return undefined;
  const time = formatTime(start, timeZone, locale);
  if (event.recurrent && event.recurrent_frequency === "WEEKLY" && (event.recurrent_interval ?? 1) === 1) {
    const weekday = new Intl.DateTimeFormat(locale, { weekday: "long", timeZone }).format(new Date(start));
    return { label: `${eventText(locale, "Every {weekday}", { weekday })} - ${time}`, dateTime: start };
  }
  if (event.recurrent && event.recurrent_frequency === "DAILY" && (event.recurrent_interval ?? 1) === 1) {
    return { label: `${eventText(locale, "Every day")} - ${time}`, dateTime: start };
  }
  return { label: `${formatDate(start, timeZone, locale)} - ${time}`, dateTime: start };
}

/** The schedule line: "Wed, Nov 11 · 5:00 PM – 7:00 PM", prefixed "Starting" for a recurring event. */
export function scheduleLine(event: DetailSchedule, timeZone: Zone, locale: EventsLocale = "en"): (EventCardTime & { recurrence?: string }) | undefined {
  const { start, finish } = eventOccurrence(event);
  if (!start || Number.isNaN(Date.parse(start))) return undefined;
  const range = `${formatDate(start, timeZone, locale)} · ${formatTime(start, timeZone, locale)}${finish ? ` – ${formatTime(finish, timeZone, locale)}` : ""}`;
  return {
    label: event.recurrent ? eventText(locale, "Starting {schedule}", { schedule: range }) : range,
    dateTime: start,
    recurrence: recurrenceLabel(event, locale),
  };
}

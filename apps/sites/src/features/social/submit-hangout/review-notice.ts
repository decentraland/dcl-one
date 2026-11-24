import { readRejectionReason, eventText, type EventsLocale } from "@dcl/one-ui";

/**
 * The rejected event's notice: its known reasons as one list ("A and B"), the request to fix them, then
 * the moderator's note, or any other stored text, on a line of its own.
 */
export function rejectedNotice(stored: string | null | undefined, locale: EventsLocale): string {
  const { reasons, note } = readRejectionReason(stored);
  const lead = reasons.length
    ? eventText(locale, "This event has been rejected because of {reasons}. Please update the information and submit again", {
      reasons: new Intl.ListFormat(locale, { type: "conjunction" }).format(reasons),
    })
    : eventText(locale, "This event has been rejected. Please update the information and submit again");
  return note ? `${lead}\n${note}` : lead;
}

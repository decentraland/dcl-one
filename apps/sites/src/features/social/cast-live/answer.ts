// The bound of a Cast join and what the pages say of a request that failed. No imports: the
// watcher route takes it without the streamer's state machine.

/** A join (token, module, room) that has not answered by then is stopped. */
export const JOIN_TIMEOUT_MS = 45_000;
/** What the page says of a request nobody answered in its bound. */
export const NO_ANSWER = "The Cast server did not answer in time";
/** What the page says of a request that failed with no words for the person. */
export const TRY_AGAIN = "Try again in a moment.";

/** What a person reads of a request that failed, from the error's text: `NO_ANSWER` as it is,
 * plain words for any other ("Catalyst returned 502 Bad Gateway"). The text itself goes to
 * telemetry and the console only. A refusal in the server's own words is not a failure. */
export function failureWords(reason: unknown): string {
  return reason === NO_ANSWER ? NO_ANSWER : TRY_AGAIN;
}

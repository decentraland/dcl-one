/** `text` with anything that looks like an API key or bearer token masked, for the log. */
export function redactSecrets(text: string): string {
  return text
    .replace(/\b(sk-|AIza)[\w-]{8,}/g, '$1<redacted>')
    .replace(/(Bearer\s+)[\w.~+/=-]+/gi, '$1<redacted>');
}

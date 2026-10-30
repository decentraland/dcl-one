// For tests that depend on the page's origin. `history.replaceState` can change a path or a query
// but never the host, so a test that needs another host moves the whole jsdom page.

const START = location.href

/** Put the page at `url`, host included. */
export function visit(url: string): void {
  ;(globalThis as unknown as { jsdom: { reconfigure(options: { url: string }): void } }).jsdom.reconfigure({ url })
}

/** Back to where the test run started. */
export function leave(): void {
  visit(START)
}

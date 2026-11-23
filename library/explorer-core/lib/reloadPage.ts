/** Load the page again, or load `url` in its place. A module of its own, so a test can replace it:
 *  jsdom has no navigation. */
export function reloadPage(url?: string): void {
  if (url) window.location.replace(url)
  else window.location.reload()
}

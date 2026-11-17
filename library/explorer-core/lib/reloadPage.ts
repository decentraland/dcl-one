/** Load the page again. A module of its own, so a test can replace it: jsdom has no navigation. */
export function reloadPage(): void {
  window.location.reload()
}

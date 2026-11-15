// The address-bar parameters that name a place: a link's, and the ones the engine writes once in
// world (rendering/bevy-explorer/source/src/web.rs update_url_params).
const DESTINATION_PARAMS = ['position', 'realm', 'world']

/** `search` without a place. A page that loads with a place and no stored sign-in starts its engine
 *  only after a sign-in and then goes past the lobby; without one it opens on the start screen. */
export function withoutDestination(search: string): string {
  const q = new URLSearchParams(search)
  for (const name of DESTINATION_PARAMS) q.delete(name)
  return q.size > 0 ? '?' + q.toString() : ''
}

/** Take the place off this page's address bar. */
export function dropDestination(): void {
  const search = withoutDestination(location.search)
  if (search !== location.search) history.replaceState(null, '', location.pathname + search + location.hash)
}

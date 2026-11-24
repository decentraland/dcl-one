/**
 * Whether page views leave out the root's `userId` for now: set the moment a sign-out starts, before
 * the server has cleared the verified-wallet cookie and root's data has reloaded without it, so no
 * page view sent in between names the account the visitor is leaving. Browser state for this
 * document only.
 */
let dropped = false;

export function dropTrackedUser(): void {
  dropped = true;
}

export function restoreTrackedUser(): void {
  dropped = false;
}

export function trackedUserDropped(): boolean {
  return dropped;
}

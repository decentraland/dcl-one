// The fake explorer's service worker, registered only by `?fake=storage`. It is the kind of worker the
// real shell has: registered at the origin's root scope, it outlives the Chrome run, and it answers every
// navigation itself with the app page (an app shell with a navigation fallback). A run that starts with
// this worker still registered never reaches the harness's reset page, which is what the harness's
// profile scrub exists to prevent.
self.addEventListener('install', () => self.skipWaiting())
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()))
self.addEventListener('fetch', (event) => {
  if (event.request.mode === 'navigate') event.respondWith(fetch('/index.html', { cache: 'no-store' }))
})

// The entry-link switches the mock bridge (engine/mockBridge.ts) reads besides `mock` itself. One
// list, so both HUD hosts accept the same ones (their lib/entryParams.ts spread it) and a link
// carrying one is not told it was ignored. Apart from the bridge so a host's entry-param table,
// which the shell page loads too, does not pull the mock's fixtures in with it. The app itself
// loads the bridge.
export const MOCK_SWITCHES = [
  // the stored account the mock starts with, and its welcome page
  'previousLogin',
  'welcome',
  // a screen held for a look at it (mockHold)
  'mockhold',
  // the flat colour behind the HUD, the empty account, the place with nobody else in it
  'mockworld',
  'mockaccount',
  'mockplayers',
  // a call, a stream and the portables in progress; the events reads failing
  'mockcall',
  'mockstream',
  'mockportables',
  // a scene holding the time of day: the Skybox menu locked
  'mocksky',
  'mockEvents'
] as const

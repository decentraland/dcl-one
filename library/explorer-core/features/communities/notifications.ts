// The community notifications about invites and join requests: clicking one opens its community's
// card, and one arriving while the Communities page is open reads its Invites & Requests again.
export const COMMUNITY_REQUEST_NOTIFICATIONS: ReadonlySet<string> = new Set([
  'community_invite_received',
  'community_request_to_join_received',
  'community_request_to_join_accepted'
])

/**
 * The event administration pages' copy. decentraland.org ships these pages in English only (its
 * `whats_on_admin` strings have no translations), so they are not part of the localized messages.
 */
export const adminCopy = {
  navigation: 'Event administration',
  tabs: { events: 'Events', pending: 'Pending Events', users: 'Users' },
  createEvent: '+ Create Event',
  pending: {
    title: 'Pending Events',
    recentlyApproved: 'Recently Approved',
    recentlyApprovedSubtitle: '(Last 24hs)',
    empty: 'No events waiting for approval',
    approve: 'Approve',
    reject: 'Reject',
    edit: 'Edit Event',
    actionError: 'Unable to update event. Please try again.',
    approveSuccess: 'Event/Experience approved successfully',
    rejectSuccess: 'Event/Experience rejected successfully',
    today: 'Today',
    tomorrow: 'Tomorrow',
    inDays: (count: number) => `In ${count} Days`,
    status: { pending: 'Pending', approved: 'Approved', rejected: 'Rejected' },
    by: 'By',
    loading: 'Loading…',
    loadError: 'The pending events could not be loaded.',
    retry: 'Retry'
  },
  users: {
    title: 'Users',
    searchLabel: 'Search',
    searchPlaceholder: 'Type wallet address',
    addUser: '+ Add User',
    columns: {
      user: 'User',
      approve_own_event: 'Approve Own Events',
      approve_any_event: 'Approve Events',
      edit_any_event: 'Edit Events',
      edit_any_profile: 'Edit Users'
    },
    granted: 'Granted',
    empty: 'No admins configured',
    noMatches: 'No matching users',
    rowsPerPage: 'Rows per page:',
    range: (from: number, to: number, total: number) => `${from}–${to} of ${total}`,
    previousPage: 'Previous page',
    nextPage: 'Next page',
    editUser: (user: string) => `Edit permissions for ${user}`,
    loading: 'Loading…',
    loadError: 'The users could not be loaded.',
    retry: 'Retry'
  },
  reject: {
    title: 'Reject Reason*',
    otherLabel: 'Other (optional)',
    otherPlaceholder: 'User will receive a notification, be as descriptive as you can.',
    cancel: 'Cancel',
    submit: 'Submit',
    required: 'Select at least one reason',
    noteCount: (length: number, limit: number) => `${length}/${limit}`,
    noteTooLong: (limit: number) =>
      `The note is too long for the chosen reasons. Keep it to ${limit} characters.`
  },
  permissions: {
    addTitle: 'Add User',
    editTitle: 'Edit User',
    close: 'Close',
    walletLabel: 'Wallet Address',
    walletPlaceholder: '0x…',
    walletInvalid: 'Enter a valid Ethereum address',
    cancel: 'Cancel',
    save: 'Save Changes',
    saveError: 'Unable to save permissions. Please try again.',
    readError: "Unable to read this wallet's current permissions. Please try again.",
    saveSuccess: 'Permissions updated',
    serverKept:
      "Permissions updated. The server kept the account's effective permissions: moderator accounts always retain every permission.",
    selfEditLocked: 'You cannot remove your own admin permission'
  }
} as const

/** The grants the Users page shows and edits, in decentraland.org's column order. */
export const ADMIN_PERMISSIONS = [
  {
    id: 'approve_own_event',
    title: 'Approve Own Events',
    description: 'Allow this user to approve events they create'
  },
  {
    id: 'approve_any_event',
    title: 'Approve Events',
    description: 'Allow this user to approve any event'
  },
  { id: 'edit_any_event', title: 'Edit Events', description: 'Allow this user to edit any event' },
  {
    id: 'edit_any_profile',
    title: 'Edit Users',
    description: 'Allow this user to manage admin permissions'
  }
] as const

export type AdminPermissionId = (typeof ADMIN_PERMISSIONS)[number]['id']

export const REJECT_REASONS = [
  {
    id: 'invalid_image',
    title: 'Invalid image',
    description: 'Does not comply with our Terms or Code of Ethics'
  },
  {
    id: 'invalid_event_name',
    title: 'Invalid event name',
    description: 'Does not comply with our Terms or Code of Ethics'
  },
  {
    id: 'inappropriate_description',
    title: 'Inappropriate description',
    description: 'Contains language that is not allowed'
  },
  {
    id: 'invalid_duration',
    title: 'Invalid duration',
    description: 'Too short or longer than 24 hours'
  },
  { id: 'invalid_location', title: 'Invalid location', description: 'Incorrect coordinates' }
] as const

export type RejectReasonId = (typeof REJECT_REASONS)[number]['id']

/** The longest rejection reason the Events API stores (its `MAX_REASON_LENGTH`). */
export const REJECTION_REASON_MAX_LENGTH = 500
const NOTE_SEPARATOR = '. '

const reasonTitles = (reasons: readonly RejectReasonId[]) =>
  REJECT_REASONS.filter((reason) => reasons.includes(reason.id))
    .map((reason) => reason.title)
    .join(', ')

/** The longest note that fits in the stored reason beside the chosen reasons' titles. */
export function rejectionNoteLimit(reasons: readonly RejectReasonId[]): number {
  const titles = reasonTitles(reasons)
  return REJECTION_REASON_MAX_LENGTH - (titles ? titles.length + NOTE_SEPARATOR.length : 0)
}

/**
 * The reason the API stores: the chosen reasons' titles, then the moderator's note (as decentraland.org).
 * The reject dialog keeps the note within `rejectionNoteLimit`, so nothing is cut here.
 */
export function rejectionReason(reasons: readonly RejectReasonId[], notes: string): string {
  return [reasonTitles(reasons), notes.trim()].filter(Boolean).join(NOTE_SEPARATOR)
}

/**
 * A stored reason read back: the known reasons' titles it starts with, and the moderator's note after them.
 * Text that does not start with known titles (a note stored without reasons, or a reason written elsewhere)
 * is all note, so only known titles are ever presented as reasons.
 */
export function readRejectionReason(stored: string | null | undefined): {
  reasons: string[]
  note: string
} {
  const text = stored?.trim() ?? ''
  if (!text) return { reasons: [], note: '' }
  const known = new Set<string>(REJECT_REASONS.map((reason) => reason.title))
  const end = text.indexOf('. ')
  const head = end === -1 ? text.replace(/\.$/, '') : text.slice(0, end)
  const titles = head.split(', ')
  if (titles.every((title) => known.has(title))) {
    return { reasons: titles, note: end === -1 ? '' : text.slice(end + 2).trim() }
  }
  return { reasons: [], note: text }
}

const MS_PER_DAY = 24 * 60 * 60 * 1000

/** "Today", "Tomorrow", "In 3 Days", or the day and month ("19 NOV") further out or in the past. */
export function relativeDateLabel(
  start: string | null | undefined,
  now: Date,
  locale?: string
): string {
  if (!start) return ''
  const date = new Date(start)
  if (Number.isNaN(date.getTime())) return ''
  const day = (value: Date) =>
    new Date(value.getFullYear(), value.getMonth(), value.getDate()).getTime()
  const days = Math.round((day(date) - day(now)) / MS_PER_DAY)
  if (days === 0) return adminCopy.pending.today
  if (days === 1) return adminCopy.pending.tomorrow
  if (days > 1 && days < 7) return adminCopy.pending.inDays(days)
  return date.toLocaleDateString(locale, { day: '2-digit', month: 'short' }).toUpperCase()
}

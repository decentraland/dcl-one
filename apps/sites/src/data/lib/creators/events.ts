// Collection timeline: the append-only events builder-server writes during publication and review
// (`GET /collections/:id/events`). The review stage, the validator's verdict and the daily retry
// allowance are all read off these rows; nothing here is stored on the client.

import { type BodyShape as BodyShapeUrn } from '@dcl/schemas'
import { BODY_SHAPE_FEMALE, BODY_SHAPE_MALE, type Item } from '@/data/lib/creators/items'
import { ValidationSeverity, type ValidationIssue } from '@/data/lib/creators/validation/types'

export const COLLECTION_EVENT_TYPES = [
  'collection.published',
  'review.human_required',
  'review.automated_started',
  'review.automated_passed',
  'review.automated_rejected',
  'review.automated_error',
  'review.appeal_requested',
  'review.assigned',
  'review.approved',
  'review.rejected',
  'changes.submitted',
  'review.comment',
  'collection.disabled'
] as const

export type CollectionEventActor = 'creator' | 'curator' | 'validator' | 'system'

export const REJECT_REASON_CODES = [
  'clipping',
  'thumbnail',
  'category_hides',
  'rigging',
  'triangle_count',
  'emote',
  'file_size',
  'textures_materials',
  'reversed_faces',
  'smart_wearable_files',
  'content_policy_ip',
  'other'
] as const
export type RejectReasonCode = (typeof REJECT_REASON_CODES)[number]

export type FindingSeverity = 'error' | 'warning'

export type BodyShape = 'male' | 'female'

/** One validator finding, as the validation job reports it back. */
export type ValidationFinding = {
  /** Rule-book id, e.g. M-01. */
  rule: string
  check?: string
  severity: FindingSeverity
  message: string
  where?: string
  bodyShape?: BodyShape
  measured?: number
  limit?: number
  fix?: string
  docs?: string
}

export type ValidationItemResult = {
  itemId: string
  contentHash: string
  /** `null` when the item could not be validated; never a pass. */
  passed: boolean | null
  findings: ValidationFinding[]
  visualSummary?: string
  /** Undecided because the validator cannot judge this kind of item, not because something failed. */
  unsupported?: boolean
  /** Why the item could not be validated. */
  error?: string
}

export type ValidationVerdict = 'passed' | 'rejected' | 'error'

/**
 * Payload of `review.automated_passed` / `review.automated_rejected` / `review.automated_error`: the validator's whole result.
 * `validationId` only reaches the committee; `collectionId` and `rulesVersion` are missing on older rows.
 */
export type ValidationVerdictPayload = {
  validationId?: string
  collectionId?: string
  verdict: ValidationVerdict
  reason?: 'unsupported'
  retryable?: boolean
  rulesVersion?: string
  items: ValidationItemResult[]
  /** Items edited while the validator ran: the verdict is about content that is already gone. */
  staleItemIds?: string[]
}

type CurationDecisionPayload = {
  rejectionReasons?: RejectReasonCode[]
  rejectionMessage?: string
}

/** `review.automated_error` reasons the server never resends: only a curator or a new request moves the review on. */
export type FinalAutomatedErrorReason = 'sweep_exhausted' | 'too_large'
/** The request never reached the validator; the server's sweep sends it again within minutes. */
const PUBLISH_FAILED_REASON = 'publish_failed'

export type CollectionEventPayload = Omit<Partial<ValidationVerdictPayload>, 'reason'> &
  CurationDecisionPayload & {
    /** `third_party` / `unsupported_items` / `validator_error` on `review.human_required`, `FinalAutomatedErrorReason` on `review.automated_error`; open for newer ones. */
    reason?: string
    trigger?: string
    itemIds?: string[]
    /** `review.automated_started`: the content hash of every item sent, so a later run can tell what changed. */
    contentHashes?: Record<string, string>
    note?: string
    assignee?: string | null
    /** `review.comment`: the text, and the item it is about when the author anchored it to one. */
    message?: string
    itemId?: string
  }

export type CollectionEvent = {
  id: string
  collectionId: string
  /** One of `COLLECTION_EVENT_TYPES`; kept open so a newer server type renders as an unknown row, not a crash. */
  type: string
  actor: CollectionEventActor
  actorAddress: string | null
  payload: CollectionEventPayload
  createdAt: number
}

export type CollectionEventsPage = {
  results: CollectionEvent[]
  total: number
  page: number
  limit: number
}

const VERDICT_TYPES = new Set(['review.automated_passed', 'review.automated_rejected'])
const HUMAN_DECISION_TYPES = new Set(['review.approved', 'review.rejected'])

export const VALIDATION_ATTEMPTS_PER_DAY = 5

/** The newest verdict the validator produced, if any. `events` newest first, as the server lists them. */
export function getLatestVerdict(
  events: CollectionEvent[]
): (CollectionEvent & { payload: ValidationVerdictPayload }) | null {
  const event = events.find(candidate => VERDICT_TYPES.has(candidate.type))
  if (!event || !Array.isArray(event.payload.items)) return null
  return {
    ...event,
    payload: {
      validationId: event.payload.validationId,
      verdict: event.payload.verdict ?? (event.type === 'review.automated_passed' ? 'passed' : 'rejected'),
      rulesVersion: event.payload.rulesVersion,
      // An older or malformed row may lack findings; every consumer reads them as a list.
      items: event.payload.items.map(item => (Array.isArray(item.findings) ? item : { ...item, findings: [] })),
      staleItemIds: event.payload.staleItemIds
    }
  }
}

export type CuratorVerdictOutcome = 'passed' | 'rejected' | 'unsupported'

export type CuratorVerdict = CollectionEvent & { payload: ValidationVerdictPayload; outcome: CuratorVerdictOutcome }

/**
 * The newest automated result a curator can open: a pass, a rejection, or a run that left items it can't judge
 * to a curator. `events` newest first.
 */
export function getCuratorVerdict(events: CollectionEvent[]): CuratorVerdict | null {
  const event = events.find(candidate => VERDICT_TYPES.has(candidate.type) || isUnsupportedItemsReview(candidate))
  if (!event || !Array.isArray(event.payload.items)) return null
  const outcome: CuratorVerdictOutcome =
    event.type === 'review.automated_passed'
      ? 'passed'
      : event.type === 'review.automated_rejected'
        ? 'rejected'
        : 'unsupported'
  return {
    ...event,
    outcome,
    payload: {
      validationId: event.payload.validationId,
      verdict:
        event.payload.verdict ?? (outcome === 'passed' ? 'passed' : outcome === 'rejected' ? 'rejected' : 'error'),
      rulesVersion: event.payload.rulesVersion,
      items: event.payload.items.map(item => (Array.isArray(item.findings) ? item : { ...item, findings: [] })),
      staleItemIds: event.payload.staleItemIds
    }
  }
}

/**
 * The newest automated result of the current review request; an older run belongs to another request, unless the
 * creator opened this one by asking a curator to look past that result. `events` newest first.
 */
export function getCurrentVerdict(
  events: CollectionEvent[],
  curation: { createdAt: number } | null
): CuratorVerdict | null {
  const latest = getCuratorVerdict(events)
  if (!latest || !curation) return null
  if (latest.createdAt >= curation.createdAt) return latest
  let appealed = false
  for (const event of events) {
    if (event.id === latest.id) break
    if (HUMAN_DECISION_TYPES.has(event.type)) return null
    if (event.type === 'review.appeal_requested') appealed = true
  }
  return appealed ? latest : null
}

export function startOfUtcDay(now: number): number {
  const date = new Date(now)
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())
}

const HOUR_MS = 3_600_000
const MINUTE_MS = 60_000

/** How long until the day's automated reviews are back (00:00 UTC), rounded up so nobody returns too early. */
export function getValidationResetIn(now = Date.now()): { unit: 'hours' | 'minutes'; count: number } {
  const left = startOfUtcDay(now) + 24 * HOUR_MS - now
  return left > HOUR_MS
    ? { unit: 'hours', count: Math.ceil(left / HOUR_MS) }
    : { unit: 'minutes', count: Math.max(1, Math.ceil(left / MINUTE_MS)) }
}

/**
 * Automated reviews used today: every run the creator caused (the publish, a retry, submitted changes; never
 * the server's own re-sends) newer than the later of today 00:00 UTC and the newest curator decision. A run
 * whose request never reached the validator doesn't count. The server counts the same way, so the counter
 * and the 429 agree. `events` newest first.
 */
export function countValidationAttempts(events: CollectionEvent[], now = Date.now()): number {
  const dayStart = startOfUtcDay(now)
  // The server records the failed send right after its start; creators get no validationId to pair them by.
  let unsentStarts = 0
  let attempts = 0
  for (const event of events) {
    if (event.createdAt < dayStart || HUMAN_DECISION_TYPES.has(event.type)) break
    if (event.type === 'review.automated_error' && event.payload.reason === PUBLISH_FAILED_REASON) unsentStarts++
    else if (event.type === 'review.automated_started') {
      if (unsentStarts > 0) unsentStarts--
      else if (event.payload.trigger !== 'sweep') attempts++
    }
  }
  return attempts
}

export function getValidationAttemptsLeft(events: CollectionEvent[], now = Date.now()): number {
  return Math.max(0, VALIDATION_ATTEMPTS_PER_DAY - countValidationAttempts(events, now))
}

export const COMMENT_EVENT_TYPE = 'review.comment'
export const COMMENT_MAX_LENGTH = 2000
export const NOTE_MAX_LENGTH = 2000

/** Events that say nothing about the review stage: an assignment, a submitted change or a comment leaves it where it was. */
const STAGE_NEUTRAL_EVENT_TYPES = new Set(['review.assigned', 'changes.submitted', COMMENT_EVENT_TYPE])

export function latestStageEvent(events: CollectionEvent[]): CollectionEvent | undefined {
  return events.find(event => !STAGE_NEUTRAL_EVENT_TYPES.has(event.type))
}

/** `review.human_required` written because the validator cannot check some items (e.g. facial-feature PNGs). */
const UNSUPPORTED_ITEMS_REASON = 'unsupported_items'

export function isUnsupportedItemsReview(event: CollectionEvent | undefined): boolean {
  return event?.type === 'review.human_required' && event.payload.reason === UNSUPPORTED_ITEMS_REASON
}

export const VALIDATOR_ERROR_REASON = 'validator_error'
export const SWEEP_EXHAUSTED_REASON = 'sweep_exhausted'
const TOO_LARGE_REASON = 'too_large'
const FINAL_AUTOMATED_ERROR_REASONS: readonly string[] = [
  SWEEP_EXHAUSTED_REASON,
  TOO_LARGE_REASON
] satisfies FinalAutomatedErrorReason[]

/** The request was too large to send: a curator reviews it, and an edit does not unblock a retry. */
export function isTooLargeError(event: CollectionEvent | undefined): boolean {
  return event?.type === 'review.automated_error' && event.payload.reason === TOO_LARGE_REASON
}

/** The validator gave up on the latest request: no verdict will come without a curator or a new request. */
export function isFinalValidationError(event: CollectionEvent | undefined): boolean {
  if (event?.type === 'review.automated_error')
    return FINAL_AUTOMATED_ERROR_REASONS.includes(event.payload.reason ?? '')
  return event?.type === 'review.human_required' && event.payload.reason === VALIDATOR_ERROR_REASON
}

/** A verdict is still to come: the validator is working, or failed and the server's sweep will resend. */
export function isValidationInProgress(events: CollectionEvent[]): boolean {
  const latest = latestStageEvent(events)
  return (
    latest?.type === 'review.automated_started' ||
    (latest?.type === 'review.automated_error' && !isFinalValidationError(latest))
  )
}

/** The appeal endpoint answers 409 while the latest event is still the open appeal. */
export function hasOpenAppeal(events: CollectionEvent[]): boolean {
  return events.find(event => event.type !== COMMENT_EVENT_TYPE)?.type === 'review.appeal_requested'
}

export type ReviewParty = 'creator' | 'curator'

/** Events by which one side of the review answers the other; the automated review and assignments are neither. */
const CONVERSATION_EVENT_TYPES = new Set([
  COMMENT_EVENT_TYPE,
  'review.rejected',
  'review.approved',
  'review.appeal_requested',
  'changes.submitted'
])

/**
 * Who owes the next answer in the creator–curator conversation: the side that did not write the newest comment.
 * `null` when the newest exchange is not a comment (a decision, an appeal or new changes already answer it).
 * `events` newest first.
 */
export function getAwaitedReply(events: CollectionEvent[]): ReviewParty | null {
  const latest = events.find(event => CONVERSATION_EVENT_TYPES.has(event.type))
  if (latest?.type !== COMMENT_EVENT_TYPE) return null
  if (latest.actor === 'curator') return 'creator'
  return latest.actor === 'creator' ? 'curator' : null
}

/** How many comments the other side wrote in a row that `party` has not answered yet. */
export function countUnansweredComments(events: CollectionEvent[], party: ReviewParty): number {
  if (getAwaitedReply(events) !== party) return 0
  let count = 0
  for (const event of events) {
    if (!CONVERSATION_EVENT_TYPES.has(event.type)) continue
    if (event.type !== COMMENT_EVENT_TYPE || event.actor === party) break
    count++
  }
  return count
}

/** The newest comment the other side wrote that `party` has not answered yet. */
export function getUnansweredComment(events: CollectionEvent[], party: ReviewParty): CollectionEvent | null {
  if (getAwaitedReply(events) !== party) return null
  return events.find(event => event.type === COMMENT_EVENT_TYPE) ?? null
}

/** The items the validator did not pass, in payload order; `null` (not checked) is listed too, never as a pass. */
export function getFailedItems(payload: ValidationVerdictPayload): ValidationItemResult[] {
  return payload.items
    .filter(item => item.passed !== true)
    .map(item => (Array.isArray(item.findings) ? item : { ...item, findings: [] }))
}

/** Items the validator explicitly failed; an unchecked (`null`) item is not counted as one. */
export function countFailedItems(items: ValidationItemResult[]): number {
  return items.filter(item => item.passed === false).length
}

/** A verdict's items split by what a curator should look at first: failures, then undecided, then passes. */
export function groupVerdictItems<T extends Pick<ValidationItemResult, 'passed'>>(
  items: T[]
): { failed: T[]; unchecked: T[]; passed: T[] } {
  return {
    failed: items.filter(item => item.passed === false),
    unchecked: items.filter(item => item.passed === null),
    passed: items.filter(item => item.passed === true)
  }
}

const BODY_SHAPE_FOLDER = /^("?)(male|female)\//i

/**
 * One row per finding, with the body shapes it was found on: the same finding on both shapes' files differs only
 * in `bodyShape` and the shape's folder in `where`. Order follows each row's first occurrence.
 */
export function mergeBodyShapeFindings<F extends { bodyShape?: BodyShape; where?: string }>(
  findings: F[]
): (Omit<F, 'bodyShape'> & { bodyShapes: BodyShape[] })[] {
  const rows = new Map<string, Omit<F, 'bodyShape'> & { bodyShapes: BodyShape[] }>()
  for (const { bodyShape, ...rest } of findings) {
    const where = bodyShape ? rest.where?.replace(BODY_SHAPE_FOLDER, '$1') : rest.where
    const shown = { ...rest, where }
    const key = JSON.stringify(shown)
    const row = rows.get(key)
    if (!row) rows.set(key, { ...shown, bodyShapes: bodyShape ? [bodyShape] : [] })
    else if (bodyShape && !row.bodyShapes.includes(bodyShape)) row.bodyShapes.push(bodyShape)
  }
  return [...rows.values()]
}

export function isRejectReasonCode(code: unknown): code is RejectReasonCode {
  return typeof code === 'string' && (REJECT_REASON_CODES as readonly string[]).includes(code)
}

const VALIDATION_TRIGGERS = ['publish', 'retry', 'changes', 'sweep'] as const
export type ValidationTrigger = (typeof VALIDATION_TRIGGERS)[number]

export function isValidationTrigger(value: unknown): value is ValidationTrigger {
  return typeof value === 'string' && (VALIDATION_TRIGGERS as readonly string[]).includes(value)
}

/** One validator finding as the item checks show it, so the automated review reads like the browser's. */
function toValidationIssue(finding: ValidationFinding): ValidationIssue {
  return {
    code: finding.rule,
    severity: finding.severity === 'error' ? ValidationSeverity.ERROR : ValidationSeverity.WARNING,
    message: finding.fix ? `${finding.message} ${finding.fix}` : finding.message,
    title: finding.check,
    where: finding.where,
    bodyShapes: finding.bodyShape
      ? [(finding.bodyShape === 'male' ? BODY_SHAPE_MALE : BODY_SHAPE_FEMALE) as BodyShapeUrn]
      : undefined
  }
}

/**
 * A finding on a body shape whose model file is the very same file as the other shape's (the usual one-model item
 * stored under both folders) is about the item, not that shape: it loses its shape and the folder in its texts, so
 * both copies merge into one.
 */
function unshapeSharedModelFinding(finding: ValidationFinding, contents: Record<string, string>): ValidationFinding {
  const match = finding.bodyShape && finding.where ? BODY_SHAPE_FOLDER.exec(finding.where) : null
  if (!match || !finding.where) return finding
  const quote = match[1]
  const tail = finding.where.slice(match[0].length)
  const path = quote && tail.endsWith(quote) ? tail.slice(0, -quote.length) : tail
  const other = `${finding.bodyShape === 'male' ? 'female' : 'male'}/${path}`
  const own = `${finding.bodyShape}/${path}`
  if (!contents[own] || contents[own] !== contents[other]) return finding
  return {
    ...finding,
    bodyShape: undefined,
    where: `${quote}${path}${quote}`,
    message: finding.message.split(own).join(path),
    ...(finding.fix ? { fix: finding.fix.split(own).join(path) } : {})
  }
}

/**
 * An item's findings as validation issues for a curator: one per finding across body shapes, without the fix. With
 * the item's `contents`, findings about a model both shapes share are shown once, for the item.
 */
export function toCuratorIssues(
  findings: ValidationFinding[],
  contents: Record<string, string> = {}
): ValidationIssue[] {
  const shown = findings.map(finding => unshapeSharedModelFinding(finding, contents))
  return mergeBodyShapeFindings(shown).map(finding => ({
    code: finding.rule,
    severity: finding.severity === 'error' ? ValidationSeverity.ERROR : ValidationSeverity.WARNING,
    message: finding.message,
    title: finding.check,
    where: finding.where,
    bodyShapes: finding.bodyShapes.map(
      shape => (shape === 'male' ? BODY_SHAPE_MALE : BODY_SHAPE_FEMALE) as BodyShapeUrn
    )
  }))
}

/**
 * Whether an item was saved or added after the automated review last judged it. The same content always gets
 * the same result, so only then is another automated review worth asking for (the server refuses it otherwise).
 */
export function hasChangesSinceVerdict(
  events: CollectionEvent[],
  items: Pick<Item, 'id' | 'currentContentHash'>[]
): boolean {
  if (!getLatestVerdict(events)) return false
  // A run only carries the items it was sent (a submitted update sends the changed ones), so each item's baseline
  // is the newest run that judged it, as builder-server compares.
  const judged = new Map<string, string>()
  for (const event of events) {
    if (!VERDICT_TYPES.has(event.type) && !isUnsupportedItemsReview(event)) continue
    if (!Array.isArray(event.payload.items)) continue
    for (const result of event.payload.items) {
      if (!judged.has(result.itemId)) judged.set(result.itemId, result.contentHash)
    }
  }
  return items.some(item => {
    if (!judged.has(item.id)) return true
    return item.currentContentHash !== undefined && item.currentContentHash !== judged.get(item.id)
  })
}

/**
 * The newest rejection's findings per item that did not pass, keyed by item id; empty once the validator
 * passed the collection or while no verdict exists. Items edited during the run or saved since are left out.
 */
export function getAutomatedIssuesByItem(
  events: CollectionEvent[],
  items: Pick<Item, 'id' | 'currentContentHash'>[]
): Map<string, ValidationIssue[]> {
  const verdict = getLatestVerdict(events)
  const issues = new Map<string, ValidationIssue[]>()
  if (!verdict || verdict.payload.verdict !== 'rejected') return issues
  // A curator approved after the run (an appeal): the findings no longer stand. A curator's rejection keeps
  // them, since they still say what to fix.
  const verdictIndex = events.findIndex(event => event.id === verdict.id)
  if (events.slice(0, verdictIndex).some(event => event.type === 'review.approved')) return issues
  const stale = new Set(verdict.payload.staleItemIds ?? [])
  // An item saved since the run carries another hash: its findings were about content that is gone.
  const currentHashes = new Map(items.map(item => [item.id, item.currentContentHash]))
  for (const result of getFailedItems(verdict.payload)) {
    const currentHash = currentHashes.get(result.itemId)
    const changed = currentHash !== undefined && currentHash !== result.contentHash
    if (stale.has(result.itemId) || changed || result.findings.length === 0) continue
    issues.set(result.itemId, result.findings.map(toValidationIssue))
  }
  return issues
}

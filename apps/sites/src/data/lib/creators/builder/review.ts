import { type CollectionEvent, type CollectionEventsPage } from '~/lib/events'
import { fromRemoteEvent } from './mappers'
import { BuilderServerError, request } from './transport'
import type { RemoteCollectionEvent, RemoteCollectionEventsPage } from './wire'

const REVIEW_CONFLICT_STATUS = 409
const COLLECTION_BUSY_STATUS = 503
const VALIDATION_LIMIT_STATUS = 429
const VALIDATION_RUNNING_MESSAGE = 'A validation is already in progress for this collection'
const COLLECTION_BUSY_MESSAGE = 'The collection is being updated, try again in a moment'

/** A 409 from the review endpoints (a run in progress, nothing new to validate, an open appeal): an answer, not a failure. */
export function isReviewConflict(error: unknown): boolean {
  return error instanceof BuilderServerError && error.status === REVIEW_CONFLICT_STATUS
}

/** The 409 that means an automated review of the collection is running right now. */
export function isValidationRunning(error: unknown): boolean {
  return isReviewConflict(error) && (error as BuilderServerError).message === VALIDATION_RUNNING_MESSAGE
}

/** builder-server is busy with another update of the collection: the same action works on retry. A gateway 503 is not. */
export function isCollectionBusy(error: unknown): boolean {
  return (
    error instanceof BuilderServerError &&
    error.status === COLLECTION_BUSY_STATUS &&
    error.message === COLLECTION_BUSY_MESSAGE
  )
}

/** Today's automated reviews are spent: the creator waits for tomorrow or asks a curator. */
export class ValidationLimitError extends BuilderServerError {
  constructor(error: BuilderServerError) {
    super(error.message, error.status, error.data)
    this.name = 'ValidationLimitError'
  }
}

/** Rethrows the daily-limit 429, the one carrying `retryAt`; the per-minute rate limit's 429 stays a failure. */
export function toValidationLimitError(error: unknown): unknown {
  const retryAt = (error as BuilderServerError | undefined)?.data as { retryAt?: unknown } | undefined
  return error instanceof BuilderServerError &&
    error.status === VALIDATION_LIMIT_STATUS &&
    typeof retryAt?.retryAt === 'string'
    ? new ValidationLimitError(error)
    : error
}

/**
 * Asks for another automated review: POST /collections/{id}/validations. 409 while one is running,
 * 429 (`ValidationLimitError`) once today's reviews are spent.
 */
export async function requestCollectionValidation(address: string, collectionId: string): Promise<void> {
  try {
    await request<unknown>(address, 'POST', `/collections/${collectionId}/validations`, '', undefined, false)
  } catch (error) {
    throw toValidationLimitError(error)
  }
}

const APPEAL_ALREADY_OPEN_MESSAGE = 'There is already an open appeal for this collection'

/** The appeal's 409 when one is already open; the other 409 means the latest request is not a rejection. */
export function isAppealAlreadyOpen(error: unknown): boolean {
  return error instanceof BuilderServerError && error.message === APPEAL_ALREADY_OPEN_MESSAGE
}

/**
 * Escalates a rejection to a human curator: POST /collections/{id}/curation/appeal. 409 while an appeal is
 * open (`isAppealAlreadyOpen`) or when the latest request is not rejected.
 */
export async function appealCollectionCuration(address: string, collectionId: string, note?: string): Promise<void> {
  const body = note ? { note } : {}
  await request<unknown>(address, 'POST', `/collections/${collectionId}/curation/appeal`, '', body, false)
}

export type CollectionComment = { message: string; itemId?: string }

/**
 * Writes on the collection's review timeline: POST /collections/{id}/comments. Open to the collection's
 * role holders and the committee; answers the stored `review.comment` event.
 */
export async function postCollectionComment(
  address: string,
  collectionId: string,
  comment: CollectionComment
): Promise<CollectionEvent> {
  const remote = await request<RemoteCollectionEvent>(
    address,
    'POST',
    `/collections/${collectionId}/comments`,
    '',
    comment
  )
  return fromRemoteEvent(remote)
}

/**
 * The collection's timeline, newest first: GET /collections/{id}/events. The server offsets by
 * `limit * (page - 1)`, so callers page with one constant `limit`.
 */
export async function fetchCollectionEvents(
  address: string,
  collectionId: string,
  { page, limit }: { page: number; limit: number }
): Promise<CollectionEventsPage> {
  const remote = await request<RemoteCollectionEventsPage>(
    address,
    'GET',
    `/collections/${collectionId}/events`,
    `?page=${page}&limit=${limit}`
  )
  return { ...remote, results: remote.results.map(fromRemoteEvent) }
}

import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest'
import { BuilderServerError } from './transport'
import {
  ValidationLimitError,
  appealCollectionCuration,
  fetchCollectionEvents,
  isAppealAlreadyOpen,
  isCollectionBusy,
  isReviewConflict,
  isValidationRunning,
  postCollectionComment,
  requestCollectionValidation
} from './review'

vi.mock('~/lib/auth', () => ({
  signedFetch: vi.fn()
}))

import { signedFetch } from '~/lib/auth'
import { ADDRESS, jsonResponse, okResponse } from '~/test/builderServer'

const signedFetchMock = signedFetch as Mock

beforeEach(() => {
  signedFetchMock.mockReset()
})

describe('collection events and review requests', () => {
  it('lists the timeline page and maps the rows', async () => {
    signedFetchMock.mockResolvedValue(
      okResponse({
        results: [
          {
            id: 'e1',
            collection_id: 'a1b2',
            type: 'review.automated_started',
            actor: 'validator',
            actor_address: null,
            payload: { trigger: 'publish' },
            created_at: '2026-09-29T10:00:00Z'
          }
        ],
        total: 1,
        pages: 1,
        page: 2,
        limit: 20
      })
    )
    const page = await fetchCollectionEvents(ADDRESS, 'a1b2', { page: 2, limit: 20 })
    expect(signedFetchMock.mock.calls[0][2]).toBe('/collections/a1b2/events?page=2&limit=20')
    expect(page.total).toBe(1)
    expect(page.results[0]).toMatchObject({ type: 'review.automated_started', payload: { trigger: 'publish' } })
  })

  it('asks for another automated review', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: true }))
    await requestCollectionValidation(ADDRESS, 'a1b2')
    expect(signedFetchMock.mock.calls[0][2]).toBe('/collections/a1b2/validations')
    expect(signedFetchMock.mock.calls[0][3]).toMatchObject({ method: 'POST' })
  })

  it('turns the daily limit into the daily-limit error', async () => {
    signedFetchMock.mockResolvedValue(
      jsonResponse({ ok: false, error: 'Too many attempts', data: { retryAt: '2026-09-30T00:00:00Z' } }, false, 429)
    )
    const error = await requestCollectionValidation(ADDRESS, 'a1b2').catch((e: unknown) => e)
    expect(error).toBeInstanceOf(ValidationLimitError)
    expect((error as ValidationLimitError).status).toBe(429)
  })

  it("keeps the per-minute rate limit's 429 apart from the daily limit", async () => {
    signedFetchMock.mockResolvedValue(
      jsonResponse({ ok: false, error: 'Too many requests, try again in a minute' }, false, 429)
    )
    const error = await requestCollectionValidation(ADDRESS, 'a1b2').catch((e: unknown) => e)
    expect(error).toBeInstanceOf(BuilderServerError)
    expect(error).not.toBeInstanceOf(ValidationLimitError)
  })

  it('keeps a running validation as a plain 409', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: false, error: 'Running' }, false, 409))
    const error = await requestCollectionValidation(ADDRESS, 'a1b2').catch((e: unknown) => e)
    expect(error).toBeInstanceOf(BuilderServerError)
    expect(error).not.toBeInstanceOf(ValidationLimitError)
    expect((error as BuilderServerError).status).toBe(409)
  })

  it('asks for a curator review', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: true }))
    await appealCollectionCuration(ADDRESS, 'a1b2')
    expect(signedFetchMock.mock.calls[0][2]).toBe('/collections/a1b2/curation/appeal')
    expect(signedFetchMock.mock.calls[0][3]).toMatchObject({ method: 'POST', body: JSON.stringify({}) })
  })

  it('sends the creator note along with the appeal', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: true }))
    await appealCollectionCuration(ADDRESS, 'a1b2', 'The cape is meant to clip')
    expect(signedFetchMock.mock.calls[0][3]).toMatchObject({
      body: JSON.stringify({ note: 'The cape is meant to clip' })
    })
  })

  it('posts a comment and answers the stored event', async () => {
    signedFetchMock.mockResolvedValue(
      okResponse({
        id: 'e9',
        collection_id: 'a1b2',
        type: 'review.comment',
        actor: 'creator',
        actor_address: '0xABC',
        payload: { message: 'Fixed the weights', itemId: 'i1' },
        created_at: '2026-11-12T10:00:00Z'
      })
    )
    const event = await postCollectionComment(ADDRESS, 'a1b2', { message: 'Fixed the weights', itemId: 'i1' })
    expect(signedFetchMock.mock.calls[0][2]).toBe('/collections/a1b2/comments')
    expect(signedFetchMock.mock.calls[0][3]).toMatchObject({
      method: 'POST',
      body: JSON.stringify({ message: 'Fixed the weights', itemId: 'i1' })
    })
    expect(event).toMatchObject({
      type: 'review.comment',
      actor: 'creator',
      actorAddress: '0xabc',
      payload: { message: 'Fixed the weights', itemId: 'i1' }
    })
  })

  it('tells an appeal already open from the other refusals', () => {
    expect(
      isAppealAlreadyOpen(new BuilderServerError('There is already an open appeal for this collection', 409))
    ).toBe(true)
    expect(isAppealAlreadyOpen(new BuilderServerError('Only rejected collections can be appealed', 409))).toBe(false)
    expect(isAppealAlreadyOpen(null)).toBe(false)
  })
})

describe('review answers', () => {
  const running = new BuilderServerError('A validation is already in progress for this collection', 409)
  const nothingNew = new BuilderServerError(
    'Nothing changed since the last validation: edit an item or appeal the decision',
    409
  )

  it('tells a running review from the other conflicts', () => {
    expect(isValidationRunning(running)).toBe(true)
    expect(isValidationRunning(nothingNew)).toBe(false)
    expect(isReviewConflict(nothingNew)).toBe(true)
    expect(isReviewConflict(new BuilderServerError('Bad request', 400))).toBe(false)
  })

  it("knows builder-server's busy collection, not a gateway that is down", () => {
    expect(
      isCollectionBusy(new BuilderServerError('The collection is being updated, try again in a moment', 503))
    ).toBe(true)
    expect(isCollectionBusy(new BuilderServerError('builder-server answered a non-JSON response', 503))).toBe(false)
  })
})

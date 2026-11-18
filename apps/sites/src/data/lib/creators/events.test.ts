import { describe, expect, it } from 'vitest'
import { BodyShape } from '@dcl/schemas'
import { ValidationSeverity } from '@/data/lib/creators/validation'
import {
  countValidationAttempts,
  getAutomatedIssuesByItem,
  getCuratorVerdict,
  getCurrentVerdict,
  countUnansweredComments,
  getAwaitedReply,
  getValidationResetIn,
  hasChangesSinceVerdict,
  getUnansweredComment,
  getFailedItems,
  getLatestVerdict,
  getValidationAttemptsLeft,
  groupVerdictItems,
  hasOpenAppeal,
  isFinalValidationError,
  isValidationInProgress,
  mergeBodyShapeFindings,
  startOfUtcDay,
  toCuratorIssues,
  type CollectionEvent,
  type ValidationFinding,
  type ValidationItemResult
} from './events'

const NOW = Date.UTC(2026, 8, 29, 15, 0, 0)
const TODAY = Date.UTC(2026, 8, 29, 0, 0, 0)

let counter = 0
function event(type: string, createdAt: number, payload: CollectionEvent['payload'] = {}): CollectionEvent {
  return { id: `e${++counter}`, collectionId: 'c1', type, actor: 'validator', actorAddress: null, payload, createdAt }
}

describe('toCuratorIssues', () => {
  it('shows a finding once across body shapes, without the fix', () => {
    const finding = {
      rule: 'M-01',
      check: 'Triangles',
      severity: 'error' as const,
      message: 'Too many',
      fix: 'Decimate'
    }
    const issues = toCuratorIssues([
      { ...finding, bodyShape: 'male' },
      { ...finding, bodyShape: 'female' },
      { rule: 'T-02', severity: 'warning', message: 'Large texture', bodyShape: 'female' }
    ])
    expect(issues).toEqual([
      expect.objectContaining({
        severity: ValidationSeverity.ERROR,
        title: 'Triangles',
        message: 'Too many',
        bodyShapes: [BodyShape.MALE, BodyShape.FEMALE]
      }),
      expect.objectContaining({ severity: ValidationSeverity.WARNING, bodyShapes: [BodyShape.FEMALE] })
    ])
  })
})

describe('toCuratorIssues with the item contents', () => {
  it('shows a finding about a model both shapes share once, for the item', () => {
    const contents = { 'male/jacket.glb': 'Qm1', 'female/jacket.glb': 'Qm1' }
    const triangles = (shape: 'male' | 'female') => ({
      rule: 'M-01',
      severity: 'warning' as const,
      message: `"${shape}/jacket.glb" has 1594 triangles`,
      where: `${shape}/jacket.glb`,
      bodyShape: shape
    })
    const issues = toCuratorIssues(
      [
        triangles('male'),
        triangles('female'),
        { rule: 'V-03', severity: 'warning', message: 'Sleeve clips', where: 'male/jacket.glb', bodyShape: 'male' }
      ],
      contents
    )
    expect(issues.map(issue => [issue.message, issue.bodyShapes])).toEqual([
      ['"jacket.glb" has 1594 triangles', []],
      ['Sleeve clips', []]
    ])
  })

  it('keeps the shape of a finding when each shape has its own model', () => {
    const contents = { 'male/jacket.glb': 'Qm1', 'female/jacket.glb': 'Qm2' }
    const issues = toCuratorIssues(
      [{ rule: 'V-03', severity: 'warning', message: 'Sleeve clips', where: 'male/jacket.glb', bodyShape: 'male' }],
      contents
    )
    expect(issues[0].bodyShapes).toEqual([BodyShape.MALE])
  })
})

describe('getCuratorVerdict', () => {
  const item = { itemId: 'i1', contentHash: 'h', passed: null, findings: [] }
  it('reads a pass, a rejection, or a run that left items to a curator', () => {
    expect(getCuratorVerdict([event('review.automated_passed', NOW, { items: [item] })])?.outcome).toBe('passed')
    expect(getCuratorVerdict([event('review.automated_rejected', NOW, { items: [item] })])?.outcome).toBe('rejected')
    expect(
      getCuratorVerdict([event('review.human_required', NOW, { reason: 'unsupported_items', items: [item] })])?.outcome
    ).toBe('unsupported')
  })

  it('has nothing to open for a hand-off without items', () => {
    expect(getCuratorVerdict([event('review.human_required', NOW, { reason: 'third_party' })])).toBeNull()
  })
})

describe('groupVerdictItems', () => {
  it('splits the items into failed, unchecked and passed, keeping their order', () => {
    const items = [
      { id: 'a', passed: true },
      { id: 'b', passed: false },
      { id: 'c', passed: null },
      { id: 'd', passed: false }
    ]
    const groups = groupVerdictItems(items)
    expect(groups.failed.map(item => item.id)).toEqual(['b', 'd'])
    expect(groups.unchecked.map(item => item.id)).toEqual(['c'])
    expect(groups.passed.map(item => item.id)).toEqual(['a'])
  })
})

describe('countUnansweredComments', () => {
  const at = (type: string, actor: CollectionEvent['actor'], createdAt: number): CollectionEvent => ({
    id: `${type}-${createdAt}`,
    collectionId: 'c1',
    type,
    actor,
    actorAddress: null,
    createdAt,
    payload: {}
  })

  it("counts the curator's comments the creator has not answered, skipping what is not conversation", () => {
    const events = [
      at('review.comment', 'curator', 5),
      at('review.assigned', 'curator', 4),
      at('review.comment', 'curator', 3),
      at('review.comment', 'creator', 2),
      at('review.comment', 'curator', 1)
    ]
    expect(countUnansweredComments(events, 'creator')).toBe(2)
    expect(countUnansweredComments(events, 'curator')).toBe(0)
  })

  it('counts nothing once a decision or new changes answered the conversation', () => {
    expect(
      countUnansweredComments([at('changes.submitted', 'creator', 2), at('review.comment', 'curator', 1)], 'creator')
    ).toBe(0)
    expect(countUnansweredComments([], 'creator')).toBe(0)
  })
})

describe('getValidationResetIn', () => {
  it('counts the hours left until 00:00 UTC, rounded up', () => {
    expect(getValidationResetIn(Date.UTC(2026, 10, 14, 1, 16))).toEqual({ unit: 'hours', count: 23 })
    expect(getValidationResetIn(Date.UTC(2026, 10, 14, 0, 0))).toEqual({ unit: 'hours', count: 24 })
  })

  it('switches to minutes in the last hour', () => {
    expect(getValidationResetIn(Date.UTC(2026, 10, 14, 23, 0))).toEqual({ unit: 'minutes', count: 60 })
    expect(getValidationResetIn(Date.UTC(2026, 10, 14, 23, 59, 30))).toEqual({ unit: 'minutes', count: 1 })
  })
})

describe('countValidationAttempts', () => {
  it('counts the runs the creator caused, never the sweep re-sends nor the verdicts', () => {
    const events = [
      event('review.automated_started', NOW, { trigger: 'sweep' }),
      event('review.automated_rejected', NOW - 1000),
      event('review.automated_started', NOW - 2000, { trigger: 'retry' }),
      event('review.automated_error', NOW - 3000),
      event('review.automated_started', NOW - 4000, { trigger: 'changes' }),
      event('review.automated_passed', NOW - 5000),
      event('review.automated_started', NOW - 6000, { trigger: 'publish' })
    ]
    expect(countValidationAttempts(events, NOW)).toBe(3)
    expect(getValidationAttemptsLeft(events, NOW)).toBe(2)
  })

  it("doesn't count a run whose request never reached the validator", () => {
    const events = [
      event('review.automated_error', NOW, { reason: 'publish_failed' }),
      event('review.automated_started', NOW - 1, { trigger: 'retry' }),
      event('review.automated_started', NOW - 1000, { trigger: 'publish' })
    ]
    expect(countValidationAttempts(events, NOW)).toBe(1)
  })

  it('reads one review used right after publishing', () => {
    const events = [
      event('review.automated_started', NOW, { trigger: 'publish' }),
      event('collection.published', NOW - 1)
    ]
    expect(countValidationAttempts(events, NOW)).toBe(1)
    expect(getValidationAttemptsLeft(events, NOW)).toBe(4)
  })

  it('resets at 00:00 UTC', () => {
    const events = [
      event('review.automated_started', TODAY + 1, { trigger: 'retry' }),
      event('review.automated_started', TODAY - 1, { trigger: 'retry' })
    ]
    expect(startOfUtcDay(NOW)).toBe(TODAY)
    expect(countValidationAttempts(events, NOW)).toBe(1)
  })

  it('resets after a curator decision', () => {
    const events = [
      event('review.automated_started', NOW - 1000, { trigger: 'retry' }),
      event('review.approved', NOW - 2000),
      event('review.automated_started', NOW - 3000, { trigger: 'retry' }),
      event('review.automated_started', NOW - 4000, { trigger: 'publish' })
    ]
    expect(countValidationAttempts(events, NOW)).toBe(1)
  })

  it('clamps the attempts left at zero', () => {
    const events = Array.from({ length: 6 }, (_, i) => event('review.automated_started', NOW - i, { trigger: 'retry' }))
    expect(getValidationAttemptsLeft(events, NOW)).toBe(0)
  })
})

describe('hasChangesSinceVerdict', () => {
  const verdict = (items: { itemId: string; contentHash: string }[]): CollectionEvent => ({
    id: 'v1',
    collectionId: 'c1',
    type: 'review.automated_rejected',
    actor: 'validator',
    actorAddress: null,
    createdAt: 10,
    payload: { verdict: 'rejected', items: items.map(item => ({ ...item, passed: false, findings: [] })) }
  })

  it('finds nothing new while every item still has the content that was judged', () => {
    const events = [verdict([{ itemId: 'i1', contentHash: 'a' }])]
    expect(hasChangesSinceVerdict(events, [{ id: 'i1', currentContentHash: 'a' }])).toBe(false)
    expect(hasChangesSinceVerdict(events, [{ id: 'i1' }])).toBe(false)
    expect(hasChangesSinceVerdict([], [{ id: 'i1', currentContentHash: 'a' }])).toBe(false)
  })

  it('finds a change once an item was saved again or added after the verdict', () => {
    const events = [verdict([{ itemId: 'i1', contentHash: 'a' }])]
    expect(hasChangesSinceVerdict(events, [{ id: 'i1', currentContentHash: 'b' }])).toBe(true)
    expect(
      hasChangesSinceVerdict(events, [
        { id: 'i1', currentContentHash: 'a' },
        { id: 'i2', currentContentHash: 'c' }
      ])
    ).toBe(true)
  })
})

describe('hasChangesSinceVerdict across runs', () => {
  const run = (id: string, createdAt: number, items: { itemId: string; contentHash: string }[]): CollectionEvent => ({
    id,
    collectionId: 'c1',
    type: 'review.automated_rejected',
    actor: 'validator',
    actorAddress: null,
    createdAt,
    payload: { verdict: 'rejected', items: items.map(item => ({ ...item, passed: false, findings: [] })) }
  })

  it('judges an item by the newest run that saw it, not only by the latest one', () => {
    // The latest run only judged the updated item; the other one was judged by the first review.
    const events = [
      run('v2', 20, [{ itemId: 'i2', contentHash: 'b' }]),
      run('v1', 10, [{ itemId: 'i1', contentHash: 'a' }])
    ]
    const items = [
      { id: 'i1', currentContentHash: 'a' },
      { id: 'i2', currentContentHash: 'b' }
    ]
    expect(hasChangesSinceVerdict(events, items)).toBe(false)
    expect(hasChangesSinceVerdict(events, [items[0], { id: 'i2', currentContentHash: 'c' }])).toBe(true)
  })
})

describe('getCurrentVerdict', () => {
  const verdict: CollectionEvent = {
    id: 'v1',
    collectionId: 'c1',
    type: 'review.automated_rejected',
    actor: 'validator',
    actorAddress: null,
    createdAt: 10,
    payload: { verdict: 'rejected', items: [] }
  }

  it('reads the result of the current request only', () => {
    expect(getCurrentVerdict([verdict], { createdAt: 5 })?.outcome).toBe('rejected')
    expect(getCurrentVerdict([verdict], { createdAt: 15 })).toBeNull()
    expect(getCurrentVerdict([verdict], null)).toBeNull()
  })

  it('keeps the result the creator asked a curator to look past, but not one a curator already decided on', () => {
    const appeal = { ...verdict, id: 'a1', type: 'review.appeal_requested', actor: 'creator' as const, createdAt: 20 }
    const decision = { ...verdict, id: 'd1', type: 'review.rejected', actor: 'curator' as const, createdAt: 15 }
    expect(getCurrentVerdict([appeal, verdict], { createdAt: 20 })?.id).toBe('v1')
    expect(getCurrentVerdict([appeal, decision, verdict], { createdAt: 20 })).toBeNull()
  })
})

describe('getAutomatedIssuesByItem', () => {
  const finding: ValidationFinding = {
    rule: 'M-01',
    check: 'Triangle count',
    severity: 'error',
    message: 'Too many triangles.',
    fix: 'Decimate the mesh.',
    bodyShape: 'male',
    where: 'male/hat.glb'
  }

  it("dresses the newest rejection's findings per failed item like the browser's checks", () => {
    const events = [
      event('review.automated_rejected', NOW, {
        verdict: 'rejected',
        items: [
          { itemId: 'i1', contentHash: 'h', passed: false, findings: [finding] },
          { itemId: 'i2', contentHash: 'h', passed: true, findings: [] },
          { itemId: 'i3', contentHash: 'h', passed: null, findings: [], unsupported: true }
        ]
      })
    ]
    const issues = getAutomatedIssuesByItem(events, [])
    expect([...issues.keys()]).toEqual(['i1'])
    expect(issues.get('i1')).toEqual([
      {
        code: 'M-01',
        severity: ValidationSeverity.ERROR,
        message: 'Too many triangles. Decimate the mesh.',
        title: 'Triangle count',
        where: 'male/hat.glb',
        bodyShapes: [BodyShape.MALE]
      }
    ])
  })

  it('shows nothing after a pass, without a verdict, or for items edited while the validator ran', () => {
    const rejected = { itemId: 'i1', contentHash: 'h', passed: false, findings: [finding] }
    expect(
      getAutomatedIssuesByItem([event('review.automated_passed', NOW, { verdict: 'passed', items: [] })], []).size
    ).toBe(0)
    expect(getAutomatedIssuesByItem([event('review.automated_started', NOW)], []).size).toBe(0)
    expect(
      getAutomatedIssuesByItem(
        [event('review.automated_rejected', NOW, { verdict: 'rejected', items: [rejected], staleItemIds: ['i1'] })],
        []
      ).size
    ).toBe(0)
  })

  it('drops the findings of an item saved since the run, and keeps them while its content is the one judged', () => {
    const rejected = [
      event('review.automated_rejected', NOW, {
        verdict: 'rejected',
        items: [{ itemId: 'i1', contentHash: 'judged', passed: false, findings: [finding] }]
      })
    ]
    expect(getAutomatedIssuesByItem(rejected, [{ id: 'i1', currentContentHash: 'fixed' }]).size).toBe(0)
    expect(getAutomatedIssuesByItem(rejected, [{ id: 'i1', currentContentHash: 'judged' }]).size).toBe(1)
    expect(getAutomatedIssuesByItem(rejected, [{ id: 'i1', currentContentHash: undefined }]).size).toBe(1)
  })

  it('reads a malformed item without findings as having none', () => {
    const events = [
      event('review.automated_rejected', NOW, {
        verdict: 'rejected',
        items: [{ itemId: 'i1', contentHash: 'h', passed: false } as unknown as ValidationItemResult]
      })
    ]
    expect(getAutomatedIssuesByItem(events, []).size).toBe(0)
  })

  it('drops the findings once a curator approves after the run, as on an appeal', () => {
    const events = [
      event('review.approved', NOW),
      event('review.appeal_requested', NOW - 1_000),
      event('review.automated_rejected', NOW - 2_000, {
        verdict: 'rejected',
        items: [{ itemId: 'i1', contentHash: 'h', passed: false, findings: [finding] }]
      })
    ]
    expect(getAutomatedIssuesByItem(events, []).size).toBe(0)
  })

  it("keeps the findings when a curator confirms the validator's rejection: they still say what to fix", () => {
    const events = [
      event('review.rejected', NOW),
      event('review.automated_rejected', NOW - 2_000, {
        verdict: 'rejected',
        items: [{ itemId: 'i1', contentHash: 'h', passed: false, findings: [finding] }]
      })
    ]
    expect([...getAutomatedIssuesByItem(events, []).keys()]).toEqual(['i1'])
  })
})

describe('getLatestVerdict', () => {
  it('answers the newest verdict with its items, and null without one', () => {
    const items = [{ itemId: 'i1', contentHash: 'h', passed: false, findings: [] }]
    const events = [
      event('review.appeal_requested', NOW),
      event('review.automated_rejected', NOW - 1000, { verdict: 'rejected', items }),
      event('review.automated_passed', NOW - 2000, { verdict: 'passed', items: [] })
    ]
    const verdict = getLatestVerdict(events)
    expect(verdict?.payload.verdict).toBe('rejected')
    expect(getFailedItems(verdict!.payload)).toEqual(items)
    expect(getLatestVerdict([event('review.automated_started', NOW)])).toBeNull()
  })

  it('reads an item without findings as one with none, so every consumer can list them', () => {
    // A row as an older server wrote it: the item carries no findings list.
    const items = [{ itemId: 'i1', contentHash: 'h', passed: false }] as unknown as ValidationItemResult[]
    const verdict = getLatestVerdict([event('review.automated_rejected', NOW, { verdict: 'rejected', items })])
    expect(verdict?.payload.items[0].findings).toEqual([])
  })

  it('fills in the verdict from the event type when the payload lacks it', () => {
    expect(getLatestVerdict([event('review.automated_passed', NOW, { items: [] })])?.payload.verdict).toBe('passed')
  })
})

describe('latest-event flags', () => {
  it('knows when a validation is running and when an appeal is open', () => {
    expect(isValidationInProgress([event('review.assigned', NOW), event('review.automated_started', NOW - 1)])).toBe(
      true
    )
    expect(isValidationInProgress([event('review.automated_rejected', NOW)])).toBe(false)
    expect(hasOpenAppeal([event('review.appeal_requested', NOW)])).toBe(true)
    expect(hasOpenAppeal([])).toBe(false)
  })

  it('keeps a verdict pending through a validator error and an assignment, until it lands', () => {
    expect(isValidationInProgress([event('review.assigned', NOW), event('review.automated_error', NOW - 1)])).toBe(true)
    expect(isValidationInProgress([event('review.automated_started', NOW)])).toBe(true)
    expect(
      isValidationInProgress([event('review.automated_rejected', NOW), event('review.automated_started', NOW - 1)])
    ).toBe(false)
  })

  it('stops waiting once the server gave up on the request', () => {
    for (const reason of ['sweep_exhausted', 'too_large']) {
      const gaveUp = event('review.automated_error', NOW, { reason, validationId: 'v1' })
      expect(isFinalValidationError(gaveUp)).toBe(true)
      expect(isValidationInProgress([event('review.assigned', NOW), gaveUp])).toBe(false)
    }
    const validatorError = event('review.human_required', NOW, { reason: 'validator_error' })
    expect(isFinalValidationError(validatorError)).toBe(true)
    expect(isFinalValidationError(event('review.human_required', NOW, { reason: 'third_party' }))).toBe(false)
    expect(isFinalValidationError(event('review.automated_error', NOW, { verdict: 'error', retryable: true }))).toBe(
      false
    )
  })
})

describe('mergeBodyShapeFindings', () => {
  const finding = { rule: 'M-01', check: 'triangles', severity: 'error' as const, message: 'Too many triangles' }

  it('merges the same finding on both body shapes into one row', () => {
    expect(
      mergeBodyShapeFindings<ValidationFinding>([
        { ...finding, where: 'male/hat.glb', bodyShape: 'male', measured: 1940, limit: 1500 },
        { ...finding, where: 'female/hat.glb', bodyShape: 'female', measured: 1940, limit: 1500 },
        { rule: 'S-05', severity: 'warning', message: 'Large file' }
      ])
    ).toEqual([
      { ...finding, where: 'hat.glb', measured: 1940, limit: 1500, bodyShapes: ['male', 'female'] },
      { rule: 'S-05', severity: 'warning', message: 'Large file', where: undefined, bodyShapes: [] }
    ])
  })

  it('keeps findings apart when their measures differ', () => {
    const rows = mergeBodyShapeFindings<ValidationFinding>([
      { ...finding, where: '"male/hat.glb" › Albedo', bodyShape: 'male', measured: 1940 },
      { ...finding, where: '"female/hat.glb" › Albedo', bodyShape: 'female', measured: 2100 }
    ])
    expect(rows.map(row => [row.where, row.bodyShapes])).toEqual([
      ['"hat.glb" › Albedo', ['male']],
      ['"hat.glb" › Albedo', ['female']]
    ])
  })
})

describe('review conversation', () => {
  const comment = (actor: CollectionEvent['actor'], createdAt: number, message = 'hi'): CollectionEvent => ({
    ...event('review.comment', createdAt, { message }),
    actor,
    actorAddress: '0xabc'
  })

  it('expects an answer from the side that did not write the newest comment', () => {
    expect(getAwaitedReply([comment('curator', NOW)])).toBe('creator')
    expect(getAwaitedReply([comment('creator', NOW), comment('curator', NOW - 1)])).toBe('curator')
    expect(getAwaitedReply([])).toBeNull()
  })

  it('takes new changes, an appeal or a decision after a comment as its answer', () => {
    for (const type of ['changes.submitted', 'review.appeal_requested', 'review.rejected', 'review.approved']) {
      expect(getAwaitedReply([event(type, NOW), comment('curator', NOW - 1)])).toBeNull()
    }
  })

  it('still expects the answer after an assignment or an automated run', () => {
    const events = [
      event('review.assigned', NOW),
      event('review.automated_started', NOW - 1),
      comment('curator', NOW - 2)
    ]
    expect(getAwaitedReply(events)).toBe('creator')
  })

  it('hands the creator the curator comment waiting for them, and nothing once answered', () => {
    const waiting = comment('curator', NOW, 'Is the tail meant to be a tiara?')
    expect(getUnansweredComment([waiting], 'creator')?.payload.message).toBe('Is the tail meant to be a tiara?')
    expect(getUnansweredComment([waiting], 'curator')).toBeNull()
    expect(getUnansweredComment([comment('creator', NOW + 1), waiting], 'creator')).toBeNull()
  })

  it('keeps an appeal open and a review running while people comment on it', () => {
    expect(hasOpenAppeal([comment('curator', NOW), event('review.appeal_requested', NOW - 1)])).toBe(true)
    expect(isValidationInProgress([comment('creator', NOW), event('review.automated_started', NOW - 1)])).toBe(true)
  })
})

// A purchase a scene asked for (library/ui/apps/shop/scenePurchase): what the shell is asked, and what the
// scene is told as the operation ends.
import { describe, expect, it, vi } from 'vitest'
import { purchaseSceneItem, sceneOutcome, type SceneShell } from '@dcl/one-ui/apps/shop/scenePurchase'
import { OPERATION_STATES, SHOP_EVENT, SHOP_METHODS } from '@dcl/one-ui/apps/shop/shopProtocol'

const CONTRACT = '0x' + 'ab'.repeat(20)
const ITEM = { contract: '0x' + 'AB'.repeat(20), itemId: '7' }
const operation = (state: string, id = 'op-1'): Record<string, unknown> => ({ v: 1, operation: id, buyer: null, state, items: [{ contract: CONTRACT, itemId: '7', quantity: 1 }] })

/** A shell whose answer to shop.buy the test gives, and whose events it posts. */
function fakeShell(): { shell: SceneShell; answer: (value: unknown) => void; refuse: (e: Error) => void; post: (message: Record<string, unknown>) => void; request: ReturnType<typeof vi.fn>; listening: () => number } {
  let answer!: (value: unknown) => void
  let refuse!: (e: Error) => void
  const handlers = new Set<(message: Record<string, unknown>) => void>()
  const request = vi.fn(
    () =>
      new Promise((resolve, reject) => {
        answer = resolve
        refuse = reject
      })
  )
  const shell: SceneShell = {
    request: request as unknown as SceneShell['request'],
    listen: (_type, handler) => {
      handlers.add(handler)
      return () => handlers.delete(handler)
    }
  }
  return { shell, answer: (v) => answer(v), refuse: (e) => refuse(e), post: (m) => handlers.forEach((h) => h({ ...m, type: SHOP_EVENT })), request, listening: () => handlers.size }
}

describe('a purchase a scene asked for', () => {
  it('tells the scene only a verdict', () => {
    expect(OPERATION_STATES.map((s) => [s, sceneOutcome(s)])).toEqual([
      ['reviewing', null],
      ['processing', null],
      ['pending', 'dismissed'],
      ['succeeded', 'purchased'],
      ['failed', 'failed'],
      ['cancelled', 'dismissed']
    ])
  })

  it('asks the shell for one unit of the item in Credits and follows its operation', async () => {
    const { shell, answer, post, request, listening } = fakeShell()
    const outcome = purchaseSceneItem(ITEM, shell)
    expect(request).toHaveBeenCalledWith(SHOP_METHODS.buy, { v: 1, items: [{ contract: CONTRACT, itemId: '7', quantity: 1 }], method: 'credits' }, expect.any(Number))
    answer(operation('reviewing'))
    await Promise.resolve()
    post(operation('succeeded', 'another'))
    post(operation('processing'))
    expect(listening()).toBe(1)
    post(operation('succeeded'))
    expect(await outcome).toBe('purchased')
    expect(listening()).toBe(0)
  })

  it('hears an end that arrives before the answer naming the operation', async () => {
    const { shell, answer, post } = fakeShell()
    const outcome = purchaseSceneItem(ITEM, shell)
    post(operation('cancelled'))
    answer(operation('reviewing'))
    expect(await outcome).toBe('dismissed')
  })

  it.each([
    ['the shell refuses', (s: ReturnType<typeof fakeShell>) => s.refuse(new Error('a purchase is already open'))],
    ['the shell answers nothing it can read', (s: ReturnType<typeof fakeShell>) => s.answer({ v: 2, operation: 'op-1' })],
    ['the shell answers nothing', (s: ReturnType<typeof fakeShell>) => s.answer(null)]
  ])('fails when %s', async (_, act) => {
    const s = fakeShell()
    const outcome = purchaseSceneItem(ITEM, s.shell)
    act(s)
    expect(await outcome).toBe('failed')
    expect(s.listening()).toBe(0)
  })
})

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook, waitFor } from '@testing-library/react'
import { BodyShape } from '@dcl/schemas'
import { TranslationProvider } from '~/intl'
import { ItemType, type Item } from '~/lib/items'
import { ValidationSeverity, type ValidationSource } from '~/lib/validation'

const stores = new Map<string, Map<string, unknown>>()
vi.mock('idb-keyval', () => ({
  createStore: (db: string, name: string) => {
    const key = `${db}/${name}`
    if (!stores.has(key)) stores.set(key, new Map())
    return key
  },
  get: async (key: string, store: string) => stores.get(store)!.get(key),
  set: async (key: string, value: unknown, store: string) => void stores.get(store)!.set(key, value),
  del: async (key: string, store: string) => void stores.get(store)!.delete(key),
  clear: async (store: string) => stores.get(store)!.clear()
}))
const validate = vi.fn()
const validateThumbnail = vi.fn()
vi.mock('~/lib/validation', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/validation')>()),
  getValidator: () => ({ validate, validateThumbnail })
}))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))
vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))

const { useCollectionValidation, useRerunItemValidation } = await import('./useCollectionValidation')

function makeItem(id: string, model: string): Item {
  return {
    id,
    name: id,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xabc',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: {
      category: 'hat',
      representations: [{ bodyShapes: [BodyShape.MALE, BodyShape.FEMALE], mainFile: 'hat.glb', contents: ['hat.glb'] }]
    },
    contents: { 'hat.glb': model, 'thumbnail.png': `thumb-${id}` },
    createdAt: 1,
    updatedAt: 1
  }
}

const error = { code: 'triangle-count', severity: ValidationSeverity.ERROR, message: 'Too many triangles' }
const warning = { code: 'textures', severity: ValidationSeverity.WARNING, message: 'Large texture' }
const broken = makeItem('broken', 'bafyBroken')
const meh = makeItem('meh', 'bafyMeh')
const clean = makeItem('clean', 'bafyClean')

function issuesFor(source: ValidationSource) {
  const hash = source.kind === 'item' ? source.item.contents['hat.glb'] : ''
  return { issues: hash === 'bafyBroken' ? [error] : hash === 'bafyMeh' ? [warning] : [] }
}

function renderValidation(items: Item[], enabled = true, client = new QueryClient()) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>
      <TranslationProvider>{children}</TranslationProvider>
    </QueryClientProvider>
  )
  return renderHook(() => ({ validation: useCollectionValidation(items, enabled), rerun: useRerunItemValidation() }), {
    wrapper
  })
}

beforeEach(() => {
  // The cache keeps its opened store for the page load, so empty it rather than dropping it.
  for (const store of stores.values()) store.clear()
  vi.stubGlobal('indexedDB', {})
  validate.mockReset().mockImplementation(async (source: ValidationSource) => issuesFor(source))
  validateThumbnail.mockReset().mockResolvedValue({ issues: [] })
})

describe('useCollectionValidation', () => {
  it('gives every item its status once its model and thumbnail are checked', async () => {
    const { result } = renderValidation([broken, meh, clean])
    expect(result.current.validation.isValidating).toBe(true)
    await waitFor(() => expect(result.current.validation.isValidating).toBe(false))
    const { results } = result.current.validation
    expect(results.get('broken')).toEqual({ status: 'errors', issues: [error] })
    expect(results.get('meh')).toEqual({ status: 'warnings', issues: [warning] })
    expect(results.get('clean')).toEqual({ status: 'pass', issues: [] })
  })

  it('checks an item without a thumbnail on its model alone', async () => {
    const bare = { ...makeItem('bare', 'bafyMeh'), contents: { 'hat.glb': 'bafyMeh' } }
    const { result } = renderValidation([bare, clean])
    await waitFor(() => expect(result.current.validation.isValidating).toBe(false))
    expect(result.current.validation.results.get('bare')).toEqual({ status: 'warnings', issues: [warning] })
    expect(validateThumbnail).toHaveBeenCalledTimes(1)
  })

  it('checks only the items it is given', async () => {
    const { result } = renderValidation([meh])
    await waitFor(() => expect(result.current.validation.isValidating).toBe(false))
    expect(validate).toHaveBeenCalledTimes(1)
    expect(result.current.validation.results.has('broken')).toBe(false)
  })

  it('checks nothing while disabled', () => {
    const { result } = renderValidation([broken, meh], false)
    expect(validate).not.toHaveBeenCalled()
    expect(result.current.validation).toEqual({ results: new Map(), isValidating: false })
  })

  it('serves saved items from the persisted results after a reload', async () => {
    const first = renderValidation([broken])
    await waitFor(() => expect(first.result.current.validation.isValidating).toBe(false))
    first.unmount()

    const second = renderValidation([broken])
    await waitFor(() => expect(second.result.current.validation.results.get('broken')?.status).toBe('errors'))
    expect(validate).toHaveBeenCalledTimes(1)
    expect(validateThumbnail).toHaveBeenCalledTimes(1)
  })

  it('re-runs an item from scratch and shows the fresh result everywhere', async () => {
    const { result } = renderValidation([broken])
    await waitFor(() => expect(result.current.validation.results.get('broken')?.status).toBe('errors'))

    validate.mockResolvedValue({ issues: [] })
    let fresh: unknown
    await act(async () => {
      fresh = await result.current.rerun(broken, 'details', 'errors')
    })
    expect(fresh).toEqual([])
    expect(validate).toHaveBeenCalledTimes(2)
    await waitFor(() => expect(result.current.validation.results.get('broken')).toEqual({ status: 'pass', issues: [] }))
  })
})

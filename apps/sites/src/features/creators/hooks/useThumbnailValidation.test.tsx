import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { ItemType, type Item } from '~/lib/items'
import { ValidationSeverity, type ThumbnailSource } from '~/lib/validation'
import { useThumbnailValidation } from './useThumbnailValidation'

const validateThumbnail = vi.fn()
vi.mock('~/lib/validation', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/validation')>()),
  getValidator: () => ({ validateThumbnail })
}))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))

const warning = { code: 'thumbnail', severity: ValidationSeverity.WARNING, message: 'Not transparent' }

function item(thumbnailHash: string): Item {
  return {
    id: 'i1',
    name: 'Hat',
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xabc',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: { category: 'hat', representations: [] },
    contents: { 'thumbnail.png': thumbnailHash },
    createdAt: 1,
    updatedAt: 1
  }
}

function renderValidation(initial: ThumbnailSource | null) {
  const client = new QueryClient()
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>
      <TranslationProvider>{children}</TranslationProvider>
    </QueryClientProvider>
  )
  return renderHook(({ source }) => useThumbnailValidation(source), { wrapper, initialProps: { source: initial } })
}

beforeEach(() => {
  validateThumbnail.mockReset().mockResolvedValue({ issues: [warning] })
})

describe('useThumbnailValidation', () => {
  it('checks a picked PNG once, and again only when another one is picked', async () => {
    const first = new Blob(['a'])
    const { result, rerender } = renderValidation({ kind: 'blob', blob: first })
    await waitFor(() => expect(result.current.data?.issues).toEqual([warning]))

    rerender({ source: { kind: 'blob', blob: first } })
    expect(validateThumbnail).toHaveBeenCalledTimes(1)

    rerender({ source: { kind: 'blob', blob: new Blob(['b']) } })
    await waitFor(() => expect(validateThumbnail).toHaveBeenCalledTimes(2))
  })

  it('re-checks a saved item only when its stored thumbnail changes', async () => {
    const { result, rerender } = renderValidation({ kind: 'item', item: item('Qm1') })
    await waitFor(() => expect(result.current.data).toBeDefined())

    rerender({ source: { kind: 'item', item: { ...item('Qm1'), name: 'Renamed' } } })
    expect(validateThumbnail).toHaveBeenCalledTimes(1)

    rerender({ source: { kind: 'item', item: item('Qm2') } })
    await waitFor(() => expect(validateThumbnail).toHaveBeenCalledTimes(2))
  })

  it('shows a check that could not run as a warning, never a pass', async () => {
    validateThumbnail.mockRejectedValue(new Error('chunk load failed'))
    const { result } = renderValidation({ kind: 'blob', blob: new Blob(['a']) })

    await waitFor(() => expect(result.current.data?.issues).toHaveLength(1))
    expect(result.current.data?.issues[0].severity).toBe(ValidationSeverity.WARNING)
  })

  it('does nothing without a thumbnail', () => {
    renderValidation(null)
    expect(validateThumbnail).not.toHaveBeenCalled()
  })
})

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { ItemType } from '~/lib/items'
import { ValidationSeverity, type ValidationSource } from '~/lib/validation'
import { useModelValidation } from './useModelValidation'

const validate = vi.fn()
vi.mock('~/lib/validation', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/validation')>()),
  getValidator: () => ({ validate })
}))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))

const source: ValidationSource = { kind: 'blob', contents: { 'hat.glb': new Blob(['x']) }, mainFile: 'hat.glb' }

function renderValidation() {
  const client = new QueryClient()
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>
      <TranslationProvider>{children}</TranslationProvider>
    </QueryClientProvider>
  )
  return renderHook(() => useModelValidation(source, { type: ItemType.WEARABLE }, 'draft-1'), { wrapper })
}

beforeEach(() => {
  validate.mockReset()
})

describe('useModelValidation', () => {
  it("returns the validator's issues", async () => {
    const issue = { code: 'triangle-count', severity: ValidationSeverity.ERROR, message: 'Too many triangles' }
    validate.mockResolvedValue({ issues: [issue] })
    const { result } = renderValidation()
    await waitFor(() => expect(result.current.data?.issues).toEqual([issue]))
  })

  it('passes the emote loop setting on to the validator', async () => {
    validate.mockResolvedValue({ issues: [] })
    const client = new QueryClient()
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>
        <TranslationProvider>{children}</TranslationProvider>
      </QueryClientProvider>
    )
    renderHook(() => useModelValidation(source, { type: ItemType.EMOTE, loop: true }, 'emote-1'), { wrapper })
    await waitFor(() => expect(validate).toHaveBeenCalled())
    expect(validate.mock.calls[0][1]).toMatchObject({ loop: true })
  })

  it('shows a model check that could not run as a warning, never a pass', async () => {
    validate.mockRejectedValue(new Error('404'))
    const { result } = renderValidation()
    await waitFor(() => expect(result.current.data?.issues).toHaveLength(1))
    expect(result.current.data?.issues[0].severity).toBe(ValidationSeverity.WARNING)
  })
})

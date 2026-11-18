import { describe, expect, it, vi } from 'vitest'

let loads = 0
vi.mock('@dcl-regenesislabs/wearable-validator', () => {
  loads++
  if (loads === 1) throw new Error('chunk failed to load')
  return {
    validate: vi.fn().mockResolvedValue({ findings: [], checks: [] }),
    checks: {}
  }
})

const { runRuleBook } = await import('./ruleBook')

describe('runRuleBook', () => {
  it('loads the rule book again after a failed load instead of failing every later run', async () => {
    const job = { kind: 'thumbnail', path: 'thumbnail.png', bytes: new Uint8Array([1]) } as const
    await expect(runRuleBook(job)).rejects.toThrow()
    await expect(runRuleBook(job)).resolves.toMatchObject({ findings: [] })
  })
})

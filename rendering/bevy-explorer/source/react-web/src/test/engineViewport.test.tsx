import { render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { EngineViewport } from '../features/engine/EngineViewport'

describe('EngineViewport', () => {
  it('reports its rect again when something holding it scrolls, not when a list beside it does', () => {
    const report = vi.fn()
    const { getByTestId } = render(
      <div data-testid="page">
        <div data-testid="list" />
        <EngineViewport region="avatarPreview" report={report} />
      </div>
    )
    const reports = (): number => report.mock.calls.filter(([, rect]) => rect !== null).length
    const before = reports()
    getByTestId('list').dispatchEvent(new Event('scroll'))
    expect(reports()).toBe(before)
    getByTestId('page').dispatchEvent(new Event('scroll'))
    expect(reports()).toBe(before + 1)
  })
})

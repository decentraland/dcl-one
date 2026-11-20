import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { WebSelect, WebTextarea } from './InputFieldWebControls'

describe('when rendering a shared web select', () => {
  it('should give the native control its class and mirror its invalid state on the frame', () => {
    const { rerender } = render(
      <WebSelect aria-label="Reason" aria-invalid="true" className="report-reason">
        <option>Harassment</option>
      </WebSelect>
    )
    const select = screen.getByRole('combobox', { name: 'Reason' })
    expect(select).toHaveClass('input', 'nativeSelect', 'report-reason')
    expect(select).toHaveAttribute('aria-invalid', 'true')
    expect(select.parentElement).toHaveAttribute('data-invalid', 'true')

    rerender(
      <WebSelect aria-label="Reason" aria-invalid="false" className="report-reason">
        <option>Harassment</option>
      </WebSelect>
    )
    expect(select.parentElement).not.toHaveAttribute('data-invalid')
  })
})

describe('when rendering a shared web textarea', () => {
  it('should give the native control its class and mirror its invalid state on the frame', () => {
    const { rerender } = render(
      <WebTextarea aria-label="Description" aria-invalid className="report-description" rows={4} />
    )
    const textarea = screen.getByRole('textbox', { name: 'Description' })
    expect(textarea).toHaveClass('input', 'nativeTextarea', 'report-description')
    expect(textarea).toHaveAttribute('rows', '4')
    expect(textarea.parentElement).toHaveAttribute('data-invalid', 'true')

    rerender(<WebTextarea aria-label="Description" aria-invalid={false} />)
    expect(textarea.parentElement).not.toHaveAttribute('data-invalid')
  })
})

import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { FieldLabel } from './FieldLabel'

describe('FieldLabel', () => {
  describe('when it has only its text', () => {
    it('is a label that names the field it points at', () => {
      render(
        <>
          <FieldLabel htmlFor="name" className="mine">
            Name
          </FieldLabel>
          <input id="name" />
        </>
      )
      expect(screen.getByLabelText('Name')).toHaveAttribute('id', 'name')
      expect(screen.getByText('Name')).toHaveClass('mine')
      expect(screen.getByText('Name').parentElement?.tagName).not.toBe('SPAN')
    })
  })

  describe('when it has a notice', () => {
    it('writes it as a superscript inside the label', () => {
      render(<FieldLabel notice="Optional">Name</FieldLabel>)
      const notice = screen.getByText('Optional')
      expect(notice.tagName).toBe('SUP')
      expect(notice.parentElement?.tagName).toBe('LABEL')
    })
  })

  describe('when it has a sublabel', () => {
    it('writes the helper line after the label, outside it', () => {
      render(<FieldLabel sublabel="Shown on your passport">Name</FieldLabel>)
      const helper = screen.getByText('Shown on your passport')
      const label = screen.getByText('Name')
      expect(label.tagName).toBe('LABEL')
      expect(label).not.toContainElement(helper)
      expect(label.nextElementSibling).toBe(helper)
    })
  })
})

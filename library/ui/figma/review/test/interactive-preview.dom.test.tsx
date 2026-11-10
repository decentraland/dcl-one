import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ComponentType } from 'react'
import { beforeAll, describe, expect, it } from 'vitest'
import { InteractivePreview } from '../src/review/InteractivePreview'
import { SearchBar } from '../src/generated/components/search-bar/SearchBar'
import { InputWebCreatorTools } from '../src/generated/components/text-input/InputWebCreatorTools'
import { restoreCaret } from '../src/review/InteractivePreview.helpers'

// The playground remounts its field whenever the Figma state changes (the field sits at a different
// depth per state), so focus and the caret are restored from the input's ref.
const preview = (name: string, component: unknown) =>
  render(
    <InteractivePreview
      name={name}
      component={component as ComponentType<Record<string, unknown>>}
      properties={{}}
    />
  )

beforeAll(() => {
  // jsdom has no layout; the playground only measures overflow with it.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver
})

describe('InteractivePreview field remount', () => {
  // The email field keeps one element across default, focus and editing in the current capture, so
  // the remount path is covered by restoreCaret below; this guards the playground itself.
  describe('when the email playground is focused and typed into', () => {
    it('should keep focus and the typed value', async () => {
      preview('InputWebCreatorTools', InputWebCreatorTools)
      const user = userEvent.setup()
      const field = screen.getByRole('textbox')
      expect(field.getAttribute('type')).toBe('email')

      await act(() => user.click(field))
      await act(() => user.type(field, 'a@b.co'))

      const current = screen.getByRole('textbox') as HTMLInputElement
      expect(document.activeElement).toBe(current)
      expect(current.value).toBe('a@b.co')
    })
  })

  describe('when the SearchBar playground changes state while focused', () => {
    it('should keep focus with the caret after the query', async () => {
      preview('SearchBar', SearchBar)
      const user = userEvent.setup()
      const field = screen.getByRole('searchbox')

      await act(() => user.click(field))
      await act(() => user.type(field, 'plaza'))

      const current = screen.getByRole('searchbox') as HTMLInputElement
      expect(document.activeElement).toBe(current)
      expect(current.value).toBe('plaza')
      expect(current.selectionStart).toBe(5)
      expect(current.selectionEnd).toBe(5)
    })
  })
})

describe('restoreCaret', () => {
  const input = (type: string, value: string) => {
    const element = document.createElement('input')
    element.type = type
    element.value = value
    document.body.append(element)
    return element
  }

  describe('when the remounted field is an email input', () => {
    it('should leave the selection alone, since email inputs reject setSelectionRange', () => {
      const element = input('email', 'a@b.co')

      let rejected: unknown
      try {
        element.setSelectionRange(6, 6)
      } catch (error) {
        rejected = error
      }
      expect(rejected).toMatchObject({ name: 'InvalidStateError' })
      expect(() => restoreCaret(element, 6)).not.toThrow()
    })
  })

  describe('when the remounted field is a search input', () => {
    it('should put the caret at the given position', () => {
      const element = input('search', 'plaza')
      element.setSelectionRange(0, 0)

      restoreCaret(element, 5)

      expect([element.selectionStart, element.selectionEnd]).toEqual([5, 5])
    })
  })
})

import { act, screen } from '@testing-library/react'
import { hydrateRoot, type Root } from 'react-dom/client'
import { renderToString } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Modal } from './Modal'

let root: Root | undefined
afterEach(() => {
  act(() => root?.unmount())
  root = undefined
  document.body.innerHTML = ''
  vi.restoreAllMocks()
})

function Page() {
  return (
    <div style={{ overflow: 'hidden' }}>
      <Modal title="Download" onClose={() => undefined}>
        <button type="button">Get it</button>
      </Modal>
    </div>
  )
}

describe('Modal hydration', () => {
  describe('when the server rendered it open', () => {
    it('should hydrate without a mismatch, then move onto document.body with focus inside', () => {
      const container = document.createElement('div')
      container.innerHTML = renderToString(<Page />)
      document.body.appendChild(container)
      const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined)

      act(() => {
        root = hydrateRoot(container, <Page />, { onRecoverableError: (error) => errors(error) })
      })

      expect(errors).not.toHaveBeenCalled()
      const dialog = screen.getByRole('dialog', { name: 'Download' })
      expect(container.contains(dialog)).toBe(false)
      expect(dialog.parentElement?.parentElement).toBe(document.body)
      expect(dialog.contains(document.activeElement)).toBe(true)
      expect(screen.getAllByRole('dialog')).toHaveLength(1)
    })
  })
})

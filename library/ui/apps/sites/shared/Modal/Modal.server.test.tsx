// @vitest-environment node
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { Modal } from './Modal'

// The server has no document, so the modal renders in place instead of through a portal. No
// consumer opens a modal on the first render today; one that does must expect this markup to be
// moved to document.body on hydration.
describe('Modal on the server', () => {
  describe('when rendered without a document', () => {
    it('should render the dialog in place inside its backdrop', () => {
      const html = renderToString(<Modal title="Download">QR</Modal>)

      expect(html).toMatch(/^<div class="[^"]*backdrop[^"]*"><div[^>]+role="dialog"/)
      expect(html).toContain('aria-modal="true"')
      expect(html).toContain('Download')
    })
  })
})

import type { ReactNode } from 'react'
// Global `submit-hangout*` classes kept for the event authoring form in
// apps/sites/src/features/social/submit-hangout, which applies them by name.
import './ldsubmithangoutview.css'

/** The page landmark of the event authoring route, with the form's stylesheet. */
function SubmitHangoutView({ children }: { children?: ReactNode }) {
  return <main className="submit-hangout-route">{children}</main>
}

export { SubmitHangoutView }

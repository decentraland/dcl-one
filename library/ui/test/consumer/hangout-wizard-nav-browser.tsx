import type { ReactNode } from 'react'
import { createRoot } from 'react-dom/client'
import { Button } from '../../components/Button/Button'
import { SubmitHangoutView } from '../../apps/sites/events/SubmitHangoutView'
import '../../foundations/foundations.css'

// The hangout wizard's step actions as apps/sites/src/features/social/submit-hangout/SubmitHangout.tsx draws
// them: shared `md` Buttons in the `submit-hangout__nav` row of the wizard's card, styled by the view's sheet.
function Step({ step, children }: { step: string; children: ReactNode }) {
  return (
    <div className="submit-hangout" data-step={step}>
      <div className="submit-hangout__controls">
        <div className="submit-hangout__nav">{children}</div>
      </div>
    </div>
  )
}

createRoot(document.getElementById('root')!).render(
  <SubmitHangoutView>
    <Step step="cover">
      <Button size="md" variant="tertiary" href="#events">
        Back
      </Button>
      <Button size="md">Continue</Button>
    </Step>
    <Step step="review">
      <Button size="md" variant="tertiary">
        Back
      </Button>
      <Button size="md" variant="tertiary">
        Preview
      </Button>
      <Button size="md">Submit Hangout</Button>
    </Step>
    <Step step="preview">
      <Button size="md" variant="tertiary">
        Back
      </Button>
      <Button size="md">Confirm &amp; Submit</Button>
    </Step>
  </SubmitHangoutView>
)

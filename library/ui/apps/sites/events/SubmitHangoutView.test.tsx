import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { SubmitHangoutView } from './SubmitHangoutView'

describe('SubmitHangoutView', () => {
  it('wraps the authoring form in the route landmark', () => {
    expect(
      renderToStaticMarkup(
        <SubmitHangoutView>
          <div className="submit-hangout">form</div>
        </SubmitHangoutView>
      )
    ).toBe('<main class="submit-hangout-route"><div class="submit-hangout">form</div></main>')
  })

  it('renders an empty landmark without children', () => {
    expect(renderToStaticMarkup(<SubmitHangoutView />)).toBe(
      '<main class="submit-hangout-route"></main>'
    )
  })
})

import { renderToString } from 'react-dom/server'
import { describe, it, expect } from 'vitest'
import { TitleBar, type TitleBarProps } from '@dcl/one-ui'
const shared = {
  sections: [{ id: 'graphics', label: 'Graphics', icon: <svg /> }],
  activeId: 'graphics',
  onSelect: () => {}
}
const cases: TitleBarProps[] = [
  { ...shared, variant: 'settings' },
  { ...shared, variant: 'backpack', controls: { collection: null, filters: null, search: null } },
  { ...shared, variant: 'events', controls: { date: null, create: null } },
  { ...shared, variant: 'places', controls: { filters: null, search: null } },
  { ...shared, variant: 'shop', controls: { filters: null, sort: null, search: null, cart: 0 } }
]
describe('when the public toolbar renders on the server', () => {
  it.each(cases)(
    'should render $variant without a generated runtime or browser globals',
    (props) => {
      const html = renderToString(<TitleBar {...props} />)
      expect(html).toContain('data-variant="' + props.variant + '"')
      expect(html).toContain('aria-pressed="true"')
      expect(html).not.toMatch(/FigmaView|figma-module-runtime|registerComponent/)
    }
  )
})

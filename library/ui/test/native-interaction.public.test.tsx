// @vitest-environment node
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { Tabs, InputExplorerNames } from '@dcl/one-ui'
describe('when unheld native components render through the public root without a browser', () => {
  it('should preserve a named current route anchor and escape its label', () => {
    const html = renderToString(<Tabs href="/profile" label="A < B" selected />)
    expect(html).toContain('href="/profile"')
    expect(html).toContain('aria-current="page"')
    expect(html).toContain('A &lt; B')
    expect(html).not.toContain('<button')
  })
  it('should preserve native password form fields without client-only initial state', () => {
    const html = renderToString(
      <InputExplorerNames
        label="Password"
        name="password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal
      />
    )
    expect(html).toContain('type="password"')
    expect(html).toContain('name="password"')
    expect(html).toContain('Show password')
  })
})

import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import type { ComponentProps } from 'react'
import { afterEach, expect, it } from 'vitest'
import { Button } from '../../index'
afterEach(cleanup)
for (const href of [undefined, '/save']) {
  const input = Object.freeze({
    children: 'Save',
    ...(href ? { href } : {}),
    dangerouslySetInnerHTML: Object.freeze({ __html: '<b data-injected="yes">Wrong content</b>' })
  })
  const untyped = input as unknown as ComponentProps<typeof Button>
  it(`should retain owned content during SSR for untyped HTML props (href=${href})`, () => {
    const html = renderToString(<Button {...untyped} />)
    expect(html).toContain('Save')
    expect(html).not.toContain('Wrong content')
    expect(html).not.toContain('data-injected')
    expect(input.dangerouslySetInnerHTML.__html).toContain('Wrong content')
  })
  it(`should render the named native control for untyped HTML props (href=${href})`, () => {
    render(<Button {...untyped} />)
    expect(screen.getByRole(href ? 'link' : 'button', { name: 'Save' })).toBeInTheDocument()
    expect(screen.queryByText('Wrong content')).not.toBeInTheDocument()
  })
}

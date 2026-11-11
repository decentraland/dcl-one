import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createRef } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Button } from '../../../index'
import { FigmaView, type ViewNode } from '../../../components/runtime/figma-module-runtime'

const views: Record<string, ViewNode> = {
  default: {
    kind: 'box',
    path: '',
    name: 'Button',
    className: 'captured',
    children: [
      { kind: 'text', path: '0', name: 'label', className: 'label', text: 'Label', slot: 'label' }
    ]
  }
}
const select = { values: {}, booleans: {}, flags: {} }
afterEach(cleanup)

describe('when a generated button accepts native control props', () => {
  it('should render a native button with its submit type and pressed state', async () => {
    const onClick = vi.fn()
    render(
      <FigmaView
        as="button"
        views={views}
        select={select}
        slots={{ label: 'Join' }}
        action={{ type: 'submit', onClick, 'aria-pressed': true }}
      />
    )
    const button = screen.getByRole('button', { name: 'Join', pressed: true })
    expect(button).toHaveAttribute('type', 'submit')
    await userEvent.click(button)
    expect(onClick).toHaveBeenCalledOnce()
  })

  it('should forward a cancellable click event to a link and secure a new window', async () => {
    const onClick = vi.fn((event) => event.preventDefault())
    render(
      <FigmaView
        as="button"
        views={views}
        select={select}
        slots={{ label: 'Explore' }}
        action={{ href: '/explore', target: '_blank', rel: 'nofollow', onClick }}
      />
    )
    const link = screen.getByRole('link', { name: 'Explore' })
    expect(link).toHaveAttribute('href', '/explore')
    expect(link).toHaveAttribute('rel', 'nofollow noopener noreferrer')
    await userEvent.click(link)
    expect(onClick.mock.calls[0]![0].defaultPrevented).toBe(true)
  })

  it('should prevent disabled links from navigating or activating', async () => {
    const onClick = vi.fn()
    render(
      <FigmaView
        as="button"
        disabled
        views={views}
        select={select}
        slots={{ label: 'Explore' }}
        action={{ href: '/explore', onClick }}
      />
    )
    const link = screen.getByRole('link', { name: 'Explore' })
    expect(link).not.toHaveAttribute('href')
    expect(link).toHaveAttribute('aria-disabled', 'true')
    expect(link).toHaveAttribute('tabindex', '-1')
    await userEvent.click(link)
    expect(onClick).not.toHaveBeenCalled()
  })

  it('should keep disabled native buttons inactive', async () => {
    const onClick = vi.fn()
    render(
      <FigmaView
        as="button"
        disabled
        views={views}
        select={select}
        slots={{ label: 'Join' }}
        action={{ onClick }}
      />
    )
    await userEvent.click(screen.getByRole('button', { name: 'Join' }))
    expect(onClick).not.toHaveBeenCalled()
  })

  it('should compose decorative icons around replacement content', () => {
    render(
      <FigmaView
        as="button"
        views={views}
        select={select}
        slots={{}}
        action={{
          children: 'Visit',
          leadingIcon: <svg data-testid="leading" />,
          trailingIcon: <svg data-testid="trailing" />,
          className: 'wide'
        }}
      />
    )
    expect(screen.getByRole('button', { name: 'Visit' })).toHaveClass('wide')
    expect(screen.getByTestId('leading').parentElement).toHaveAttribute('aria-hidden', 'true')
    expect(screen.getByTestId('trailing').parentElement).toHaveAttribute('aria-hidden', 'true')
    expect(screen.queryByText('Label')).not.toBeInTheDocument()
  })

  it('should require an accessible name for an icon-only control', () => {
    expect(() =>
      render(
        <FigmaView
          as="button"
          views={views}
          select={select}
          slots={{}}
          action={{ iconOnly: true, leadingIcon: <svg /> }}
        />
      )
    ).toThrow('iconOnly requires an accessible name')
  })

  it('should omit captured text in a named icon-only control', () => {
    render(
      <FigmaView
        as="button"
        views={views}
        select={select}
        slots={{}}
        action={{ iconOnly: true, 'aria-label': 'Close', leadingIcon: <svg /> }}
      />
    )
    expect(screen.getByRole('button', { name: 'Close' })).toBeInTheDocument()
    expect(screen.queryByText('Label')).not.toBeInTheDocument()
  })
})

describe('when using the generated Button contract', () => {
  it.each(['primary', 'secondary', 'tertiary', 'quaternary', 'inverse'] as const)(
    'should render %s with consumer content and a leading icon',
    (variant) => {
      render(
        <Button variant={variant} size="md" leadingIcon={<svg />} className="wide">
          Join the world
        </Button>
      )
      expect(screen.getByRole('button', { name: 'Join the world' })).toHaveClass('wide')
    }
  )

  it('should accept a named icon-only variant', () => {
    render(
      <Button variant="iconOnly" aria-label="Close">
        <svg />
      </Button>
    )
    expect(screen.getByRole('button', { name: 'Close' })).toHaveTextContent('')
  })

  it('should forward a typed, cancellable anchor click', async () => {
    const onClick = vi.fn()
    render(
      <Button
        href="/jump"
        onClick={(event) => {
          event.preventDefault()
          onClick(event.currentTarget.href)
        }}
      >
        Jump
      </Button>
    )
    await userEvent.click(screen.getByRole('link', { name: 'Jump' }))
    expect(onClick).toHaveBeenCalledWith(expect.stringContaining('/jump'))
  })

  it('should preserve an empty href and an existing safe rel without duplicates', () => {
    render(
      <Button href="" target="_blank" rel="noopener  nofollow noreferrer">
        Jump
      </Button>
    )
    expect(screen.getByText('Jump').closest('a')).toHaveAttribute('href', '')
    expect(screen.getByText('Jump').closest('a')).toHaveAttribute(
      'rel',
      'noopener nofollow noreferrer'
    )
  })

  it('should preserve aria-labelledby as an icon-only accessible name', () => {
    render(
      <>
        <span id="close-label">Close panel</span>
        <Button iconOnly aria-labelledby="close-label" leadingIcon={<svg />} />
      </>
    )
    expect(screen.getByRole('button', { name: 'Close panel' })).toBeInTheDocument()
  })
})

describe('when a paging control is unavailable but must keep keyboard focus', () => {
  it('should retain focus after becoming aria-disabled and suppress activation', async () => {
    const onClick = vi.fn()
    const { rerender } = render(<Button onClick={onClick}>Next</Button>)
    await userEvent.tab()
    const button = screen.getByRole('button', { name: 'Next' })
    expect(button).toHaveFocus()
    rerender(
      <Button aria-disabled onClick={onClick}>
        Next
      </Button>
    )
    expect(button).toHaveFocus()
    expect(button).not.toBeDisabled()
    await userEvent.keyboard('{Enter} ')
    await userEvent.click(button)
    expect(onClick).not.toHaveBeenCalled()
    expect(button).toHaveFocus()
  })

  it('should keep an aria-disabled link focusable while suppressing navigation and auxiliary activation', async () => {
    const onClick = vi.fn()
    const onAuxClick = vi.fn()
    render(
      <Button href="/next" aria-disabled="true" onClick={onClick} onAuxClick={onAuxClick}>
        Next
      </Button>
    )
    const link = screen.getByRole('link', { name: 'Next' })
    expect(link).toHaveAttribute('href', '/next')
    await userEvent.tab()
    expect(link).toHaveFocus()
    await userEvent.click(link)
    await userEvent.pointer({ keys: '[MouseMiddle]', target: link })
    expect(onClick).not.toHaveBeenCalled()
    expect(onAuxClick).not.toHaveBeenCalled()
  })

  it('should activate when aria-disabled is false', async () => {
    const onClick = vi.fn()
    render(
      <Button aria-disabled="false" onClick={onClick}>
        Next
      </Button>
    )
    await userEvent.click(screen.getByRole('button', { name: 'Next' }))
    expect(onClick).toHaveBeenCalledOnce()
  })
})

describe('when a public Button is busy or unavailable', () => {
  it('should retain its name, expose busy state and block repeat submits while loading', async () => {
    const onClick = vi.fn()
    const onSubmit = vi.fn((event) => event.preventDefault())
    render(
      <form onSubmit={onSubmit}>
        <Button type="submit" loading onClick={onClick}>
          Save changes
        </Button>
      </form>
    )
    const button = screen.getByRole('button', { name: 'Save changes' })
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(button).toBeDisabled()
    await userEvent.click(button)
    expect(onClick).not.toHaveBeenCalled()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('should keep a loading link named and inactive', async () => {
    const onClick = vi.fn()
    render(
      <Button href="/save" loading onClick={onClick}>
        Save changes
      </Button>
    )
    const link = screen.getByRole('link', { name: 'Save changes' })
    expect(link).toHaveAttribute('aria-busy', 'true')
    expect(link).not.toHaveAttribute('href')
    await userEvent.click(link)
    expect(onClick).not.toHaveBeenCalled()
  })

  it('should prevent an aria-disabled submit button from submitting its form', async () => {
    const onSubmit = vi.fn((event) => event.preventDefault())
    render(
      <form onSubmit={onSubmit}>
        <Button type="submit" aria-disabled>
          Save
        </Button>
      </form>
    )
    await userEvent.tab()
    await userEvent.keyboard('{Enter} ')
    expect(onSubmit).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'Save' })).toHaveFocus()
  })

  it('should prevent keyboard activation of an aria-disabled link', async () => {
    const onClick = vi.fn()
    render(
      <Button href="/next" aria-disabled onClick={onClick}>
        Next
      </Button>
    )
    await userEvent.tab()
    await userEvent.keyboard('{Enter}')
    expect(onClick).not.toHaveBeenCalled()
    expect(screen.getByRole('link', { name: 'Next' })).toHaveFocus()
  })

  it.each(['_blank', '_BLANK', 'help'])('should harden the %s browsing context', (target) => {
    render(
      <Button href="/help" target={target}>
        Help
      </Button>
    )
    expect(screen.getByRole('link', { name: 'Help' })).toHaveAttribute('rel', 'noopener noreferrer')
  })

  it('should preserve explicit same-context rel semantics', () => {
    render(
      <Button href="/help" target="_SELF" rel="nofollow">
        Help
      </Button>
    )
    expect(screen.getByRole('link', { name: 'Help' })).toHaveAttribute('rel', 'nofollow')
  })

  it('should not render a specimen placeholder when content is supplied only as an accessible name', () => {
    render(<Button aria-label="Continue" />)
    expect(screen.getByRole('button', { name: 'Continue' })).toHaveTextContent('')
  })

  it('should preserve compact children alongside supplied icons', () => {
    render(
      <Button iconOnly aria-label="Close" leadingIcon={<svg data-testid="first" />}>
        <svg data-testid="second" />
      </Button>
    )
    expect(screen.getByTestId('first')).toBeInTheDocument()
    expect(screen.getByTestId('second')).toBeInTheDocument()
  })
})

describe('when the public button state changes', () => {
  it('should retain the same named control across ready, loading and ready renders', async () => {
    const onClick = vi.fn()
    const { rerender } = render(<Button onClick={onClick}>Save changes</Button>)
    const button = screen.getByRole('button', { name: 'Save changes' })
    await userEvent.tab()
    expect(button).toHaveFocus()
    rerender(
      <Button loading onClick={onClick}>
        Save changes
      </Button>
    )
    expect(screen.getByRole('button', { name: 'Save changes' })).toBe(button)
    expect(button).toBeDisabled()
    await userEvent.keyboard('{Enter} ')
    await userEvent.pointer({ keys: '[MouseMiddle]', target: button })
    expect(onClick).not.toHaveBeenCalled()
    rerender(<Button onClick={onClick}>Save changes</Button>)
    expect(screen.getByRole('button', { name: 'Save changes' })).toBe(button)
    expect(button).not.toBeDisabled()
    expect(button).not.toHaveAttribute('aria-busy', 'true')
    await userEvent.click(button)
    expect(onClick).toHaveBeenCalledOnce()
  })

  it('should block keyboard and auxiliary activation of a loading link', async () => {
    const onClick = vi.fn()
    const onAuxClick = vi.fn()
    const { rerender } = render(
      <Button href="/save" onClick={onClick} onAuxClick={onAuxClick}>
        Save changes
      </Button>
    )
    await userEvent.tab()
    const link = screen.getByRole('link', { name: 'Save changes' })
    rerender(
      <Button href="/save" loading onClick={onClick} onAuxClick={onAuxClick}>
        Save changes
      </Button>
    )
    await userEvent.keyboard('{Enter}')
    await userEvent.pointer({ keys: '[MouseMiddle]', target: link })
    expect(onClick).not.toHaveBeenCalled()
    expect(onAuxClick).not.toHaveBeenCalled()
    rerender(
      <Button href="/save" onClick={onClick} onAuxClick={onAuxClick}>
        Save changes
      </Button>
    )
    expect(screen.getByRole('link', { name: 'Save changes' })).toBe(link)
    expect(link).toHaveAttribute('href', '/save')
  })

  it('should keep the loading view when explicit disabled state is also present', () => {
    render(
      <Button disabled loading>
        Save changes
      </Button>
    )
    expect(screen.getByRole('button', { name: 'Save changes' })).toHaveAttribute(
      'aria-busy',
      'true'
    )
    expect(document.querySelector('[data-loading-indicator]')).toBeInTheDocument()
  })

  it('should expose the actual native elements through typed refs', () => {
    const button = createRef<HTMLButtonElement>()
    const link = createRef<HTMLAnchorElement>()
    render(
      <>
        <Button ref={button}>Save</Button>
        <Button href="/next" ref={link}>
          Next
        </Button>
      </>
    )
    expect(button.current).toBe(screen.getByRole('button', { name: 'Save' }))
    expect(link.current).toBe(screen.getByRole('link', { name: 'Next' }))
  })
})

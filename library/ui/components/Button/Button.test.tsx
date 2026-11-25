import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { createRef, type ComponentProps, type FormEvent, type RefObject } from 'react'
import { renderToString } from 'react-dom/server'
import { Button } from './Button'

afterEach(cleanup)

afterEach(() => {
  vi.resetAllMocks()
})

const preventSubmit = () => vi.fn((event: FormEvent) => event.preventDefault())

describe('when a native Button becomes unavailable', () => {
  describe('and a focused submit button starts loading', () => {
    let submit: Mock

    beforeEach(() => {
      submit = preventSubmit()
    })

    it('should retain focus and its accessible name while loading without submitting', async () => {
      const { rerender } = render(
        <form onSubmit={submit}>
          <Button type="submit">Join</Button>
        </form>
      )
      const control = screen.getByRole('button', { name: 'Join' })
      control.focus()
      rerender(
        <form onSubmit={submit}>
          <Button type="submit" loading>
            Join
          </Button>
        </form>
      )
      expect(control).toHaveFocus()
      expect(control).not.toBeDisabled()
      expect(control).toHaveAttribute('aria-busy', 'true')
      await userEvent.keyboard('{Enter} ')
      fireEvent.click(control)
      expect(submit).not.toHaveBeenCalled()
      expect(control).toHaveAccessibleName('Join')
    })
  })

  describe('and a paging control is aria-disabled', () => {
    let onClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
    })

    it('should retain focus and suppress activation', async () => {
      render(
        <Button aria-disabled onClick={onClick}>
          Next
        </Button>
      )
      const control = screen.getByRole('button', { name: 'Next' })
      await userEvent.tab()
      expect(control).toHaveFocus()
      await userEvent.keyboard('{Enter} ')
      fireEvent.click(control)
      expect(onClick).not.toHaveBeenCalled()
    })
  })

  describe('and a focused link starts and then stops loading', () => {
    let onClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
    })

    it('should remove link navigation while retaining focus and then restore it', () => {
      const { rerender } = render(
        <Button href="/play" onClick={onClick}>
          Play
        </Button>
      )
      const control = screen.getByRole('link', { name: 'Play' })
      control.focus()
      rerender(
        <Button href="/play" onClick={onClick} loading>
          Play
        </Button>
      )
      expect(control).toHaveFocus()
      expect(control).not.toHaveAttribute('href')
      expect(control).toHaveAttribute('tabindex', '0')
      fireEvent.click(control)
      fireEvent(control, new MouseEvent('auxclick', { bubbles: true, cancelable: true, button: 1 }))
      expect(onClick).not.toHaveBeenCalled()
      rerender(
        <Button href="/play" onClick={onClick}>
          Play
        </Button>
      )
      expect(control).toHaveAttribute('href', '/play')
      expect(control).toHaveFocus()
    })
  })

  describe('and a link targets a named window and cancels its click', () => {
    let onClick: Mock

    beforeEach(() => {
      onClick = vi.fn((event: Event) => event.preventDefault())
    })

    it('should preserve the cancellable native link event and a safe rel', () => {
      render(
        <Button href="/play" target="viewer" rel="external" onClick={onClick}>
          Play
        </Button>
      )
      const link = screen.getByRole('link', { name: 'Play' })
      expect(fireEvent.click(link)).toBe(false)
      expect(onClick).toHaveBeenCalledOnce()
      expect(link).toHaveAttribute('rel', 'external noopener noreferrer')
    })
  })

  describe('and its content is explicitly null', () => {
    it('should never substitute captured specimen text', () => {
      render(<Button aria-label="Next">{null}</Button>)
      expect(screen.getByRole('button', { name: 'Next' })).not.toHaveTextContent('Label')
    })
  })

  describe('and it receives start and end icon slots', () => {
    beforeEach(() => {
      render(
        <Button startIcon={<svg data-testid="start" />} endIcon={<svg data-testid="end" />}>
          Join
        </Button>
      )
    })

    it('should keep the accessible name from its label', () => {
      expect(screen.getByRole('button', { name: 'Join' })).toBeInTheDocument()
    })

    it('should expose both slots as decorative icons', () => {
      expect(screen.getByTestId('start').parentElement).toHaveAttribute('aria-hidden', 'true')
      expect(screen.getByTestId('end').parentElement).toHaveAttribute('aria-hidden', 'true')
    })
  })
})

describe('when using the native Button contract', () => {
  it.each(['primary', 'secondary', 'secondaryLight', 'tertiary', 'quaternary', 'inverse'] as const)(
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

  describe('and the icon-only variant is named', () => {
    it('should render a named control without text content', () => {
      render(
        <Button variant="iconOnly" aria-label="Close">
          <svg />
        </Button>
      )
      expect(screen.getByRole('button', { name: 'Close' })).toHaveTextContent('')
    })
  })

  describe('and an anchor click is canceled by the caller', () => {
    let onClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
    })

    it('should forward a typed anchor click with its href', async () => {
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
  })

  describe('and the href is empty and the rel is already safe', () => {
    beforeEach(() => {
      render(
        <Button href="" target="_blank" rel="noopener  nofollow noreferrer">
          Jump
        </Button>
      )
    })

    it('should preserve the empty href', () => {
      expect(screen.getByText('Jump').closest('a')).toHaveAttribute('href', '')
    })

    it('should preserve the existing rel without duplicates', () => {
      expect(screen.getByText('Jump').closest('a')).toHaveAttribute(
        'rel',
        'noopener nofollow noreferrer'
      )
    })
  })

  describe('and an icon-only button is labelled by another element', () => {
    it('should use aria-labelledby as its accessible name', () => {
      render(
        <>
          <span id="close-label">Close panel</span>
          <Button iconOnly aria-labelledby="close-label" leadingIcon={<svg />} />
        </>
      )
      expect(screen.getByRole('button', { name: 'Close panel' })).toBeInTheDocument()
    })
  })
})

describe('when a paging control is unavailable but must keep keyboard focus', () => {
  describe('and a focused button becomes aria-disabled', () => {
    let onClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
    })

    it('should retain focus and suppress activation', async () => {
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
  })

  describe('and a link is aria-disabled', () => {
    let onClick: Mock
    let onAuxClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
      onAuxClick = vi.fn()
    })

    it('should keep the link focusable while suppressing navigation and auxiliary activation', async () => {
      render(
        <Button href="/next" aria-disabled="true" onClick={onClick} onAuxClick={onAuxClick}>
          Next
        </Button>
      )
      const link = screen.getByRole('link', { name: 'Next' })
      expect(link).not.toHaveAttribute('href')
      await userEvent.tab()
      expect(link).toHaveFocus()
      await userEvent.click(link)
      await userEvent.pointer({ keys: '[MouseMiddle]', target: link })
      expect(onClick).not.toHaveBeenCalled()
      expect(onAuxClick).not.toHaveBeenCalled()
    })
  })

  describe('and aria-disabled is false', () => {
    let onClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
    })

    it('should activate once on click', async () => {
      render(
        <Button aria-disabled="false" onClick={onClick}>
          Next
        </Button>
      )
      await userEvent.click(screen.getByRole('button', { name: 'Next' }))
      expect(onClick).toHaveBeenCalledOnce()
    })
  })
})

describe('when a public Button is busy or unavailable', () => {
  describe('and a submit button is loading', () => {
    let onClick: Mock
    let onSubmit: Mock

    beforeEach(() => {
      onClick = vi.fn()
      onSubmit = preventSubmit()
    })

    it('should retain its name, expose busy state and block repeat submits', async () => {
      render(
        <form onSubmit={onSubmit}>
          <Button type="submit" loading onClick={onClick}>
            Save changes
          </Button>
        </form>
      )
      const button = screen.getByRole('button', { name: 'Save changes' })
      expect(button).toHaveAttribute('aria-busy', 'true')
      expect(button).not.toBeDisabled()
      await userEvent.click(button)
      expect(onClick).not.toHaveBeenCalled()
      expect(onSubmit).not.toHaveBeenCalled()
    })
  })

  describe('and a link is loading', () => {
    let onClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
    })

    it('should keep the link named and inactive', async () => {
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
  })

  describe('and a submit button is aria-disabled', () => {
    let onSubmit: Mock

    beforeEach(() => {
      onSubmit = preventSubmit()
    })

    it('should prevent it from submitting its form and keep its focus', async () => {
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
  })

  describe('and a link is aria-disabled', () => {
    let onClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
    })

    it('should prevent keyboard activation and keep its focus', async () => {
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
  })

  it.each(['_blank', '_BLANK', 'help'])('should harden the %s browsing context', (target) => {
    render(
      <Button href="/help" target={target}>
        Help
      </Button>
    )
    expect(screen.getByRole('link', { name: 'Help' })).toHaveAttribute('rel', 'noopener noreferrer')
  })

  describe('and a same-context link has an explicit rel', () => {
    it('should preserve the explicit rel semantics', () => {
      render(
        <Button href="/help" target="_SELF" rel="nofollow">
          Help
        </Button>
      )
      expect(screen.getByRole('link', { name: 'Help' })).toHaveAttribute('rel', 'nofollow')
    })
  })

  describe('and content is supplied only as an accessible name', () => {
    it('should not render a specimen placeholder', () => {
      render(<Button aria-label="Continue" />)
      expect(screen.getByRole('button', { name: 'Continue' })).toHaveTextContent('')
    })
  })

  describe('and a compact button receives children alongside supplied icons', () => {
    it('should render both the icon and the children', () => {
      render(
        <Button iconOnly aria-label="Close" leadingIcon={<svg data-testid="first" />}>
          <svg data-testid="second" />
        </Button>
      )
      expect(screen.getByTestId('first')).toBeInTheDocument()
      expect(screen.getByTestId('second')).toBeInTheDocument()
    })
  })
})

describe('when the public button state changes', () => {
  describe('and a button goes through ready, loading and ready renders', () => {
    let onClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
    })

    it('should retain the same named control and activate only in the ready render', async () => {
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
      expect(button).not.toBeDisabled()
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
  })

  describe('and a link goes through a loading render', () => {
    let onClick: Mock
    let onAuxClick: Mock

    beforeEach(() => {
      onClick = vi.fn()
      onAuxClick = vi.fn()
    })

    it('should block keyboard and auxiliary activation while loading and restore the same link', async () => {
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
  })

  describe('and explicit disabled state is present while loading', () => {
    beforeEach(() => {
      render(
        <Button disabled loading>
          Save changes
        </Button>
      )
    })

    it('should expose the busy state', () => {
      expect(screen.getByRole('button', { name: 'Save changes' })).toHaveAttribute(
        'aria-busy',
        'true'
      )
    })

    it('should keep the loading indicator', () => {
      expect(document.querySelector('[data-loading-indicator]')).toBeInTheDocument()
    })
  })

  describe('and typed refs are passed', () => {
    let button: RefObject<HTMLButtonElement | null>
    let link: RefObject<HTMLAnchorElement | null>

    beforeEach(() => {
      button = createRef<HTMLButtonElement>()
      link = createRef<HTMLAnchorElement>()
      render(
        <>
          <Button ref={button}>Save</Button>
          <Button href="/next" ref={link}>
            Next
          </Button>
        </>
      )
    })

    it('should expose the actual native button through its ref', () => {
      expect(button.current).toBe(screen.getByRole('button', { name: 'Save' }))
    })

    it('should expose the actual native anchor through its ref', () => {
      expect(link.current).toBe(screen.getByRole('link', { name: 'Next' }))
    })
  })
})

describe('when an untyped caller passes dangerouslySetInnerHTML', () => {
  const injected = '<b data-injected="yes">Wrong content</b>'
  const untypedProps = (href: string | undefined) =>
    Object.freeze({
      children: 'Save',
      ...(href ? { href } : {}),
      dangerouslySetInnerHTML: Object.freeze({ __html: injected })
    }) as unknown as ComponentProps<typeof Button>

  describe.each([undefined, '/save'])('and the href is %s', (href) => {
    let props: ComponentProps<typeof Button>

    beforeEach(() => {
      props = untypedProps(href)
    })

    it('should render its own label during SSR', () => {
      const html = renderToString(<Button {...props} />)
      expect(html).toContain('Save')
      expect(html).not.toContain('Wrong content')
      expect(html).not.toContain('data-injected')
    })

    it('should render the named native control', () => {
      const { container } = render(<Button {...props} />)
      expect(screen.getByRole(href ? 'link' : 'button', { name: 'Save' })).toBeInTheDocument()
      expect(screen.queryByText('Wrong content')).not.toBeInTheDocument()
      expect(container.querySelector('[data-injected]')).toBeNull()
    })

    it("should leave the caller's props unchanged", () => {
      render(<Button {...props} />)
      expect(
        (props as unknown as { dangerouslySetInnerHTML: { __html: string } })
          .dangerouslySetInnerHTML
      ).toEqual({ __html: injected })
    })
  })
})

describe('when a Button takes the hud appearance', () => {
  describe('and it renders a button', () => {
    it('should default to the primary variant and the md size, with no radius override', () => {
      render(<Button appearance="hud">Save</Button>)
      const button = screen.getByRole('button', { name: 'Save' })
      expect(button).toHaveAttribute('type', 'button')
      expect(button).toHaveAttribute('data-variant', 'primary')
      expect(button).toHaveAttribute('data-size', 'md')
      expect(button).not.toHaveAttribute('data-radius')
      expect(button).not.toHaveAttribute('appearance')
    })

    it("should carry its variant, size and radius and keep the caller's class", () => {
      render(
        <Button appearance="hud" variant="ghost" size="bare" radius="pill" className="mine">
          Close
        </Button>
      )
      const button = screen.getByRole('button', { name: 'Close' })
      expect(button).toHaveAttribute('data-variant', 'ghost')
      expect(button).toHaveAttribute('data-size', 'bare')
      expect(button).toHaveAttribute('data-radius', 'pill')
      expect(button).toHaveClass('mine')
    })

    it('should lay its children out as its own children, with no wrapper', () => {
      render(
        <Button appearance="hud">
          <i data-testid="glyph" />
          <span>Jump in</span>
        </Button>
      )
      const button = screen.getByRole('button', { name: 'Jump in' })
      expect(screen.getByTestId('glyph').parentElement).toBe(button)
      expect(button.querySelector('[data-button-label]')).toBeNull()
    })

    it('should name itself from label when it has no children', () => {
      render(<Button appearance="hud" label="Retry" />)
      expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument()
    })

    it('should not answer a click while disabled', async () => {
      const onClick = vi.fn()
      render(
        <Button appearance="hud" disabled onClick={onClick}>
          Send
        </Button>
      )
      const button = screen.getByRole('button', { name: 'Send' })
      expect(button).toBeDisabled()
      await userEvent.click(button)
      expect(onClick).not.toHaveBeenCalled()
    })

    it('should stay enabled and answer a click while only announced as disabled', async () => {
      const onClick = vi.fn()
      render(
        <Button appearance="hud" aria-disabled="true" onClick={onClick}>
          Send
        </Button>
      )
      const button = screen.getByRole('button', { name: 'Send' })
      expect(button).not.toBeDisabled()
      await userEvent.click(button)
      expect(onClick).toHaveBeenCalledTimes(1)
    })
  })

  describe('and it renders a link', () => {
    it('should render an anchor with the same look', () => {
      render(
        <Button appearance="hud" href="/shop" size="sm">
          Shop
        </Button>
      )
      const link = screen.getByRole('link', { name: 'Shop' })
      expect(link).toHaveAttribute('href', '/shop')
      expect(link).toHaveAttribute('data-size', 'sm')
      expect(link).not.toHaveAttribute('type')
    })

    it('should protect a link that opens another tab', () => {
      render(
        <Button appearance="hud" href="https://example.com" target="_blank" rel="noopener">
          Open
        </Button>
      )
      expect(screen.getByRole('link', { name: 'Open' })).toHaveAttribute(
        'rel',
        'noopener noreferrer'
      )
    })

    it('should drop the destination of a disabled link', () => {
      render(
        <Button appearance="hud" href="/shop" disabled>
          Shop
        </Button>
      )
      const link = screen.getByText('Shop')
      expect(link).not.toHaveAttribute('href')
      expect(link).toHaveAttribute('aria-disabled', 'true')
    })
  })
})

describe('when a Button names the default appearance', () => {
  it('should render as it does without the prop', () => {
    render(<Button appearance="default">Join</Button>)
    const button = screen.getByRole('button', { name: 'Join' })
    expect(button).toHaveAttribute('data-size', 'xl')
    expect(button).not.toHaveAttribute('appearance')
    expect(button.querySelector('[data-button-label]')).not.toBeNull()
  })
})

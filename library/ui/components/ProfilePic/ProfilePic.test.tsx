import { fireEvent, render, screen } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { ConnectionStatus } from './ConnectionStatus'
import { ProfilePic } from './ProfilePic'

describe('ProfilePic', () => {
  describe('when rendered with a picture', () => {
    it('shows the picture with the member name as its text alternative', () => {
      render(<ProfilePic name="Billyjuice" src="https://faces.example/billy.png" />)
      expect(screen.getByRole('img', { name: 'Billyjuice' })).toHaveAttribute(
        'src',
        'https://faces.example/billy.png'
      )
    })

    it('takes another text alternative', () => {
      render(<ProfilePic name="Billyjuice" alt="Billyjuice's profile" src="https://faces.example/b.png" />)
      expect(screen.getByRole('img', { name: "Billyjuice's profile" })).toBeInTheDocument()
    })

    it('is decoration with an empty alternative, beside a visible name', () => {
      render(<ProfilePic name="Billyjuice" alt="" src="https://faces.example/b.png" />)
      expect(screen.queryByRole('img')).toBeNull()
      expect(document.querySelector('img')).toHaveAttribute('alt', '')
    })

    it('renders on the server without a browser', () => {
      const html = renderToString(<ProfilePic name="Billyjuice" src="https://faces.example/b.png" size={40} />)
      expect(html).toContain('alt="Billyjuice"')
      expect(html).toContain('data-size="40"')
    })
  })

  describe('when there is no picture to show', () => {
    it('shows the initial of the name, under the same text alternative', () => {
      render(<ProfilePic name="billyjuice" />)
      const fallback = screen.getByRole('img', { name: 'billyjuice' })
      expect(fallback).toHaveTextContent('B')
      expect(document.querySelector('img')).toBeNull()
    })

    it('shows the initial when the picture fails to load', () => {
      render(<ProfilePic name="Ana" src="https://faces.example/missing.png" />)
      fireEvent.error(screen.getByRole('img', { name: 'Ana' }))
      expect(screen.getByRole('img', { name: 'Ana' })).toHaveTextContent('A')
      expect(document.querySelector('img')).toBeNull()
    })

    it('tries a new picture after the earlier one failed', () => {
      const { rerender } = render(<ProfilePic name="Ana" src="https://faces.example/missing.png" />)
      fireEvent.error(screen.getByRole('img', { name: 'Ana' }))
      rerender(<ProfilePic name="Ana" src="https://faces.example/ana.png" />)
      expect(screen.getByRole('img', { name: 'Ana' })).toHaveAttribute('src', 'https://faces.example/ana.png')
    })

    it('keeps one whole character for a name that starts with an emoji, and a mark for no name', () => {
      const { rerender } = render(<ProfilePic name="🦊fox" />)
      expect(screen.getByRole('img', { name: '🦊fox' })).toHaveTextContent('🦊')
      rerender(<ProfilePic name="" alt="Unknown member" />)
      expect(screen.getByRole('img', { name: 'Unknown member' })).toHaveTextContent('?')
    })

    it('hides the initial from assistive technology with an empty alternative', () => {
      render(<ProfilePic name="Ana" alt="" />)
      expect(screen.queryByRole('img')).toBeNull()
      expect(screen.getByText('A')).toHaveAttribute('aria-hidden', 'true')
    })
  })

  describe('when the member has a connection status', () => {
    it('names the status beside the picture', () => {
      render(<ProfilePic name="Ana" src="https://faces.example/ana.png" status="online" />)
      expect(screen.getByRole('img', { name: 'Online' })).toBeInTheDocument()
      expect(screen.getByRole('img', { name: 'Ana' })).toBeInTheDocument()
    })

    it('takes the status text of another language', () => {
      render(<ProfilePic name="Ana" status="away" statusLabel="Ausente" />)
      expect(screen.getByRole('img', { name: 'Ausente' })).toHaveAttribute('data-status', 'away')
    })

    it('shows no status below 32px, as the design draws none', () => {
      render(<ProfilePic name="Ana" size={28} status="online" />)
      expect(screen.queryByRole('img', { name: 'Online' })).toBeNull()
    })
  })

  describe('when given the member name colour', () => {
    it('passes it to the styles as a custom property', () => {
      render(<ProfilePic name="Ana" nameColor="rgb(1, 2, 3)" data-testid="pic" />)
      expect(screen.getByTestId('pic').style.getPropertyValue('--profile-pic-name-color')).toBe(
        'rgb(1, 2, 3)'
      )
    })
  })
})

describe('ConnectionStatus', () => {
  it.each([
    ['online', 'Online'],
    ['away', 'Away']
  ] as const)('names the %s dot', (status, name) => {
    render(<ConnectionStatus status={status} />)
    expect(screen.getByRole('img', { name })).toHaveAttribute('data-status', status)
  })

  it('is hidden from assistive technology with an empty label', () => {
    render(<ConnectionStatus status="online" label="" data-testid="dot" />)
    expect(screen.queryByRole('img')).toBeNull()
    expect(screen.getByTestId('dot')).toHaveAttribute('aria-hidden', 'true')
  })
})

describe('when a ProfilePic takes the hud appearance', () => {
  it('should size itself from its diameter and show the picture', () => {
    const { container } = render(<ProfilePic appearance="hud" name="Mojito" src="/mojito.png" size={53} />)
    const root = container.firstElementChild as HTMLElement
    expect(root.style.width).toBe('53px')
    expect(root.style.height).toBe('53px')
    expect(root.style.fontSize).toBe('22px')
    expect(root).not.toHaveAttribute('data-size')
    expect(screen.getByRole('img', { name: 'Mojito' })).toHaveAttribute('src', '/mojito.png')
  })

  it('should show the first letter or digit past a leading @ or #, with no picture role', () => {
    const { container } = render(
      <>
        <ProfilePic appearance="hud" name="@mojito" />
        <ProfilePic appearance="hud" name="#7 lounge" />
        <ProfilePic appearance="hud" name="" />
      </>
    )
    expect([...container.children].map((node) => node.textContent)).toEqual(['M', '7', '?'])
    expect(screen.queryByRole('img')).toBeNull()
  })

  it('should paint the name colour and keep the caller class and style', () => {
    const { container } = render(
      <ProfilePic appearance="hud" name="Mojito" nameColor="rgb(1, 2, 3)" className="mine" style={{ margin: 4 }} />
    )
    const root = container.firstElementChild as HTMLElement
    expect(root.style.background).toBe('rgb(1, 2, 3)')
    expect(root.style.margin).toBe('4px')
    expect(root).toHaveClass('mine')
  })

  it('should draw a status dot scaled to the diameter, smaller at the top', () => {
    const { container } = render(
      <>
        <ProfilePic appearance="hud" name="a" size={40} status="online" />
        <ProfilePic appearance="hud" name="b" size={40} status="offline" dotPosition="top" />
        <ProfilePic appearance="hud" name="c" size={20} status="away" />
        <ProfilePic appearance="hud" name="d" size={40} />
      </>
    )
    const dots = [...container.children].map((node) => node.querySelector('span') as HTMLElement | null)
    expect(dots.map((dot) => dot?.style.width)).toEqual(['7px', '6px', '6px', undefined])
    // The ring is outside the dot's size whatever reset the host loads.
    expect(dots.slice(0, 3).map((dot) => dot?.style.boxSizing)).toEqual(['content-box', 'content-box', 'content-box'])
    expect(dots[0]!.className).toMatch(/hudOnline/)
    expect(dots[1]!.className).toMatch(/hudOffline/)
    expect(dots[1]!.className).toMatch(/hudDotTop/)
    expect(dots[2]!.className).toMatch(/hudAway/)
  })

  it('should report a picture that loads or fails and keep showing it', () => {
    const onPictureLoad = vi.fn()
    const onPictureError = vi.fn()
    render(
      <ProfilePic
        appearance="hud"
        name="Mojito"
        src="/mojito.png"
        onPictureLoad={onPictureLoad}
        onPictureError={onPictureError}
      />
    )
    const picture = screen.getByRole('img', { name: 'Mojito' })
    fireEvent.load(picture)
    fireEvent.error(picture)
    expect(onPictureLoad).toHaveBeenCalledTimes(1)
    expect(onPictureError).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('img', { name: 'Mojito' })).toBeInTheDocument()
  })

  it('should mark a framed picture', () => {
    const { container } = render(<ProfilePic appearance="hud" name="Mojito" framed />)
    expect(container.firstElementChild!.className).toMatch(/hudFramed/)
  })
})

describe('when a default ProfilePic reports its picture', () => {
  it('should call the handlers and fall back to the initial after a failure', () => {
    const onPictureLoad = vi.fn()
    const onPictureError = vi.fn()
    render(
      <ProfilePic name="Mojito" src="/mojito.png" onPictureLoad={onPictureLoad} onPictureError={onPictureError} />
    )
    const picture = screen.getByAltText('Mojito')
    fireEvent.load(picture)
    fireEvent.error(picture)
    expect(onPictureLoad).toHaveBeenCalledTimes(1)
    expect(onPictureError).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('img', { name: 'Mojito' }).tagName).toBe('SPAN')
  })
})

describe('when a default ProfilePic drops the ring', () => {
  it('should mark the root without a ring and keep the ring by default', () => {
    const { container, rerender } = render(<ProfilePic name="Ana" size={56} ring={false} />)
    expect(container.firstElementChild).toHaveAttribute('data-ring', 'none')
    expect(container.firstElementChild).toHaveAttribute('data-size', '56')
    rerender(<ProfilePic name="Ana" size={56} />)
    expect(container.firstElementChild).not.toHaveAttribute('data-ring')
  })
})

describe('when a default ProfilePic uses the gradient fallback', () => {
  it('should show up to two initials from 48px up, under the text alternative', () => {
    render(<ProfilePic name=" jane   doe smith " alt="Jane" size={56} fallback="gradient" />)
    const fallback = screen.getByRole('img', { name: 'Jane' })
    expect(fallback).toHaveTextContent(/^JD$/)
    expect(fallback.closest('[data-fallback]')).toHaveAttribute('data-fallback', 'gradient')
  })

  it('should show one initial for a one-word name', () => {
    render(<ProfilePic name="Catalyrst" size={48} fallback="gradient" />)
    expect(screen.getByRole('img', { name: 'Catalyrst' })).toHaveTextContent(/^C$/)
  })

  it('should leave the disc blank below 48px and keep it decorative with an empty alternative', () => {
    const { container } = render(<ProfilePic name="Jane Doe" alt="" size={28} fallback="gradient" />)
    expect(screen.queryByRole('img')).toBeNull()
    expect(container.textContent).toBe('')
  })

  it('should show nothing for a blank name instead of a mark', () => {
    const { container } = render(<ProfilePic name="   " alt="" size={56} fallback="gradient" />)
    expect(container.textContent).toBe('')
  })

  it('should show the picture over the gradient, and the initials after it fails to load', () => {
    const { container } = render(
      <ProfilePic name="Jane Doe" alt="" src="/jane.png" size={56} ring={false} fallback="gradient" />
    )
    const picture = container.querySelector('img')!
    expect(picture).toHaveAttribute('alt', '')
    fireEvent.error(picture)
    expect(container.querySelector('img')).toBeNull()
    expect(container.textContent).toBe('JD')
  })

  it('should keep the name initial and no gradient by default', () => {
    const { container } = render(<ProfilePic name="jane doe" size={56} />)
    expect(container.firstElementChild).not.toHaveAttribute('data-fallback')
    expect(screen.getByRole('img', { name: 'jane doe' })).toHaveTextContent(/^J$/)
  })
})

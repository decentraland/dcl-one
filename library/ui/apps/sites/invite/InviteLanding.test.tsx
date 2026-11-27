import { act, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { InviteLanding, type InviteLandingProps } from './InviteLanding'

const JOIN = '/auth/login?referrer=0xabc&redirectTo=%2Fdownload%3Freferrer%3D0xabc'
const FAQS = 'https://docs.example/player/faqs'

type Entry = { isIntersecting: boolean; intersectionRatio: number }

function renderLanding(props: Partial<InviteLandingProps> = {}) {
  return render(
    <InviteLanding referrerName="bevynaut2" joinHref={JOIN} faqsHref={FAQS} {...props} />
  )
}

describe('when the invite landing renders', () => {
  describe('and the inviter has a name', () => {
    it('should lead the heading with the name', () => {
      renderLanding()
      expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(
        'bevynaut2 invited you to join Decentraland'
      )
    })
  })

  describe('and the inviter has no name', () => {
    it('should show the title alone, as decentraland.org does', () => {
      renderLanding({ referrerName: null })
      expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(
        'invited you to join Decentraland'
      )
    })
  })

  it('should link both heroes to the sign-in it is given', () => {
    renderLanding()
    expect(screen.getByRole('link', { name: 'JOIN NOW' })).toHaveAttribute('href', JOIN)
    expect(screen.getByRole('link', { name: 'JOIN FREE' })).toHaveAttribute('href', JOIN)
  })

  it('should draw the avatar it is given in the first hero only', () => {
    const { container } = renderLanding({ avatar: <canvas data-testid="avatar" /> })
    const avatar = screen.getByTestId('avatar')
    expect(container.querySelectorAll('section')[0]).toContainElement(avatar)
    expect(screen.getAllByTestId('avatar')).toHaveLength(1)
  })

  it('should keep the avatar box empty without an avatar', () => {
    const { container } = renderLanding({ avatar: undefined })
    const boxes = container.querySelectorAll('[data-invite-avatar]')
    expect(Array.from(boxes, (box) => box.childElementCount)).toEqual([0, 0])
  })

  it('should send "see more" to the questions page it is given', () => {
    renderLanding()
    expect(screen.getByRole('link', { name: 'see more' })).toHaveAttribute('href', FAQS)
  })
})

describe('when a visitor uses the questions', () => {
  const question = (name: string) =>
    within(screen.getByRole('region', { name: 'Frequently Asked Questions' })).getByRole('button', {
      name: new RegExp(name)
    })

  it('should open a question on click and close it on a second click', async () => {
    renderLanding()
    const item = question('What is a NAME')
    await userEvent.click(item)
    expect(item).toHaveAttribute('aria-expanded', 'true')
    await userEvent.click(item)
    expect(item).toHaveAttribute('aria-expanded', 'false')
  })

  it('should keep one question open at a time', async () => {
    renderLanding()
    await userEvent.click(question('What is a NAME'))
    await userEvent.click(question('What is Decentraland'))
    expect(question('What is a NAME')).toHaveAttribute('aria-expanded', 'false')
    expect(question('What is Decentraland')).toHaveAttribute('aria-expanded', 'true')
  })

  it('should toggle with Enter and Space and ignore other keys', () => {
    renderLanding()
    const item = question('What is a NAME')
    fireEvent.keyDown(item, { key: 'Tab' })
    expect(item).toHaveAttribute('aria-expanded', 'false')
    fireEvent.keyDown(item, { key: 'Enter' })
    expect(item).toHaveAttribute('aria-expanded', 'true')
    fireEvent.keyDown(item, { key: ' ' })
    expect(item).toHaveAttribute('aria-expanded', 'false')
  })
})

describe('when the second hero decides whether to play its clip', () => {
  afterEach(() => vi.unstubAllGlobals())

  const stubObserver = () => {
    const observed: ((entries: Entry[]) => void)[] = []
    vi.stubGlobal(
      'IntersectionObserver',
      class {
        constructor(private readonly callback: (entries: Entry[]) => void) {}
        observe() {
          observed.push(this.callback)
        }
        disconnect() {}
      }
    )
    return (entry: Entry) => act(() => observed.forEach((callback) => callback([entry])))
  }

  // A window `width` px wide, with the clip's IntersectionObserver reporting `inView` once observed.
  const setup = (width: number, { inView = true, reduced = false } = {}) => {
    vi.stubGlobal('matchMedia', (query: string) => {
      const min = /min-width:\s*(\d+)px/.exec(query)
      const matches = query.includes('reduce') ? reduced : min ? width >= Number(min[1]) : false
      return { matches, media: query, addEventListener() {}, removeEventListener() {} }
    })
    const report = stubObserver()
    const { container } = renderLanding()
    report({ isIntersecting: inView, intersectionRatio: inView ? 0.5 : 0 })
    return container.querySelector('video')!
  }

  it('should load and play the clip once it is in view on a desktop-wide screen', () => {
    const video = setup(1440)
    expect(video.getAttribute('src')).toMatch(/referral\.webm$/)
    expect(video.autoplay).toBe(true)
  })

  it('should keep the poster and load nothing below the 1024px desktop breakpoint', () => {
    const video = setup(1000)
    expect(video.hasAttribute('src')).toBe(false)
    expect(video.getAttribute('poster')).toMatch(/referral-poster\.webp$/)
    expect(video.getAttribute('preload')).toBe('none')
  })

  it('should load nothing for a visitor who prefers reduced motion', () => {
    expect(setup(1440, { reduced: true }).hasAttribute('src')).toBe(false)
  })

  it('should load nothing while the clip is out of view', () => {
    expect(setup(1440, { inView: false }).hasAttribute('src')).toBe(false)
  })

  it('should load nothing while the clip only touches the bottom edge of the viewport', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: !query.includes('reduce'),
      media: query,
      addEventListener() {},
      removeEventListener() {}
    }))
    const report = stubObserver()
    const { container } = renderLanding()
    report({ isIntersecting: true, intersectionRatio: 0 })
    expect(container.querySelector('video')!.hasAttribute('src')).toBe(false)
  })

  it('should keep the poster where the browser has no IntersectionObserver', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: !query.includes('reduce'),
      media: query,
      addEventListener() {},
      removeEventListener() {}
    }))
    vi.stubGlobal('IntersectionObserver', undefined)
    const { container } = renderLanding()
    expect(container.querySelector('video')!.hasAttribute('src')).toBe(false)
  })

  describe('and width or motion change after the clip has started', () => {
    // Live media queries: `set` changes one and notifies its listeners, as a browser does.
    const live = (width: number, reduced: boolean) => {
      const state = { width, reduced }
      const listeners = new Set<() => void>()
      vi.stubGlobal('matchMedia', (query: string) => ({
        get matches() {
          const min = /min-width:\s*(\d+)px/.exec(query)
          return query.includes('reduce')
            ? state.reduced
            : min
              ? state.width >= Number(min[1])
              : false
        },
        media: query,
        addEventListener: (_: string, listener: () => void) => listeners.add(listener),
        removeEventListener: (_: string, listener: () => void) => listeners.delete(listener)
      }))
      return (next: Partial<typeof state>) => {
        Object.assign(state, next)
        act(() => listeners.forEach((listener) => listener()))
      }
    }
    const mount = () => {
      const report = stubObserver()
      const { container } = renderLanding()
      const video = container.querySelector('video')!
      // As in a browser, the loaded resource (currentSrc) outlives a removed src attribute until load() runs.
      let current = ''
      const play = vi.fn(() => {
        current = video.getAttribute('src') ?? ''
        return Promise.resolve()
      })
      const pause = vi.fn()
      const load = vi.fn(() => {
        current = video.getAttribute('src') ?? ''
      })
      Object.assign(video, { play, pause, load })
      Object.defineProperty(video, 'currentSrc', { get: () => current })
      const scroll = (inView: boolean) =>
        report({ isIntersecting: inView, intersectionRatio: inView ? 0.5 : 0 })
      return { video, play, pause, load, scroll }
    }

    it('should stop the clip and show the poster when reduced motion turns on', () => {
      const set = live(1440, false)
      const { video, pause, load, scroll } = mount()
      scroll(true)
      expect(video.getAttribute('src')).toMatch(/referral\.webm$/)
      set({ reduced: true })
      expect(video.hasAttribute('src')).toBe(false)
      expect(pause).toHaveBeenCalled()
      expect(load).toHaveBeenCalled()
    })

    it('should stop the clip when the window narrows below 1024px', () => {
      const set = live(1440, false)
      const { video, pause, scroll } = mount()
      scroll(true)
      set({ width: 900 })
      expect(video.hasAttribute('src')).toBe(false)
      expect(pause).toHaveBeenCalled()
    })

    it('should resume when motion is welcome again while the clip is in view', () => {
      const set = live(1440, false)
      const { video, play, scroll } = mount()
      scroll(true)
      set({ reduced: true })
      play.mockClear()
      set({ reduced: false })
      expect(video.getAttribute('src')).toMatch(/referral\.webm$/)
      expect(play).toHaveBeenCalled()
    })

    it('should stay on the poster when motion is welcome again after the clip left the view', () => {
      const set = live(1440, false)
      const { video, scroll } = mount()
      scroll(true)
      set({ reduced: true })
      scroll(false)
      set({ reduced: false })
      expect(video.hasAttribute('src')).toBe(false)
    })
  })
})

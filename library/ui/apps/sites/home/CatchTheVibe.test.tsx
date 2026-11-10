import { act, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { CatchTheVibe } from './CatchTheVibe'

const labels = { play: 'Play', pause: 'Pause', mute: 'Mute', unmute: 'Unmute' }
const carouselLabels = {
  previous: 'Previous slide',
  next: 'Next slide',
  goTo: (slide: number) => `Go to slide ${slide}`,
  position: (slide: number, total: number) => `Slide ${slide} of ${total}`
}
const items = [
  {
    id: 'alan',
    image: '/alan.webp',
    video: '/alan.mp4',
    creator: { name: 'AlanHowick', href: '/profile/0xd82d' }
  }
]

// The desktop row; the carousel below holds a second copy of the current clip.
const row = () => within(screen.getByRole('list'))

let play: ReturnType<typeof vi.fn<() => Promise<void>>>

// Hover-capable pointer; `previewAllowed` false means the visitor prefers reduced motion. The
// returned reduceMotion() flips the preference live and notifies listeners.
function setMedia(previewAllowed: boolean) {
  let reduced = !previewAllowed
  const listeners = new Set<() => void>()
  vi.stubGlobal(
    'matchMedia',
    vi.fn((query: string) => ({
      get matches() {
        return query.includes('no-preference') ? !reduced : reduced
      },
      media: query,
      addEventListener: (_: string, listener: () => void) => listeners.add(listener),
      removeEventListener: (_: string, listener: () => void) => listeners.delete(listener)
    }))
  )
  return {
    reduceMotion() {
      reduced = true
      for (const listener of listeners) listener()
    }
  }
}

beforeEach(() => {
  play = vi.fn<() => Promise<void>>(() => Promise.resolve())
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(play)
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined)
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('CatchTheVibe', () => {
  describe('when rendered', () => {
    it('should be a named section with a creator link and a decorative video', () => {
      setMedia(false)
      const { container } = render(
        <CatchTheVibe
          title="Catch the Vibe"
          items={items}
          labels={labels}
          carouselLabels={carouselLabels}
        />
      )

      expect(screen.getByRole('region', { name: 'Catch the Vibe' })).toBeInTheDocument()
      expect(row().getByRole('link', { name: 'AlanHowick' })).toHaveAttribute(
        'href',
        '/profile/0xd82d'
      )
      expect(container.querySelector('video')).toHaveAttribute('aria-hidden', 'true')
    })

    it('should show the clip length on the play control once the metadata loads, as source', () => {
      setMedia(false)
      const { container } = render(
        <CatchTheVibe title="t" items={items} labels={labels} carouselLabels={carouselLabels} />
      )

      const playButton = row().getByRole('button', { name: 'Play: AlanHowick' })
      expect(playButton).toHaveTextContent('0:00')
      const video = container.querySelector('video')!
      Object.defineProperty(video, 'duration', { value: 22.4, configurable: true })
      fireEvent(video, new Event('loadedmetadata'))
      expect(playButton).toHaveTextContent('0:22')
    })

    it('should keep the creator bar inside the clip card, under the media', () => {
      setMedia(false)
      render(
        <CatchTheVibe title="t" items={items} labels={labels} carouselLabels={carouselLabels} />
      )

      const card = row().getByRole('listitem')
      const creator = row().getByRole('link', { name: 'AlanHowick' })
      expect(card.lastElementChild).toBe(creator.parentElement)
      // Without a profile picture, source's stand-in face, decorative next to the named link.
      expect(creator.parentElement!.querySelector('img')).toHaveAttribute('alt', '')
    })
  })

  describe('when played from the keyboard', () => {
    it('should toggle the clip, report the pressed state and offer mute while playing', async () => {
      setMedia(false)
      const user = userEvent.setup()
      render(
        <CatchTheVibe title="t" items={items} labels={labels} carouselLabels={carouselLabels} />
      )

      const toggle = row().getByRole('button', { name: 'Play: AlanHowick' })
      expect(toggle).toHaveAttribute('aria-pressed', 'false')
      expect(row().queryByRole('button', { name: /mute/i })).not.toBeInTheDocument()

      await user.tab()
      expect(toggle).toHaveFocus()
      await user.keyboard('{Enter}')
      expect(play).toHaveBeenCalledTimes(1)
      expect(row().getByRole('button', { name: 'Pause: AlanHowick' })).toHaveAttribute(
        'aria-pressed',
        'true'
      )
      // Played on request, with sound.
      expect(row().getByRole('button', { name: 'Mute' })).toBeInTheDocument()

      await user.keyboard('{Enter}')
      expect(row().getByRole('button', { name: 'Play: AlanHowick' })).toBeInTheDocument()
    })
  })

  describe('when the pointer hovers a clip', () => {
    it('should preview it muted where hover previews are allowed', () => {
      setMedia(true)
      render(
        <CatchTheVibe title="t" items={items} labels={labels} carouselLabels={carouselLabels} />
      )

      fireEvent.mouseEnter(row().getByRole('listitem'))
      expect(play).toHaveBeenCalledTimes(1)
      expect(row().getByRole('button', { name: 'Unmute' })).toBeInTheDocument()
    })

    it('should not play when the visitor prefers reduced motion', () => {
      setMedia(false)
      render(
        <CatchTheVibe title="t" items={items} labels={labels} carouselLabels={carouselLabels} />
      )

      fireEvent.mouseEnter(row().getByRole('listitem'))
      expect(play).not.toHaveBeenCalled()
    })
  })
  describe('when a Hang Out action is given', () => {
    it('should render it as a button under the clips', async () => {
      setMedia(false)
      const onClick = vi.fn()
      const user = userEvent.setup()
      render(
        <CatchTheVibe
          title="t"
          items={items}
          labels={labels}
          carouselLabels={carouselLabels}
          hangOut={{ label: 'HANG OUT NOW', onClick }}
        />
      )

      const button = screen.getByRole('button', { name: 'HANG OUT NOW' })
      // Source's arrow mark follows the label, decorative.
      expect(button.querySelector('svg[aria-hidden="true"]')).not.toBeNull()
      await user.click(button)
      expect(onClick).toHaveBeenCalledTimes(1)
    })
  })
  describe('when reduced motion turns on during playback', () => {
    it('should stop a preview the pointer started', () => {
      const media = setMedia(true)
      const pause = vi.spyOn(HTMLMediaElement.prototype, 'pause')
      render(
        <CatchTheVibe title="t" items={items} labels={labels} carouselLabels={carouselLabels} />
      )

      fireEvent.mouseEnter(row().getByRole('listitem'))
      expect(row().getByRole('button', { name: 'Pause: AlanHowick' })).toBeInTheDocument()
      act(() => media.reduceMotion())
      expect(pause).toHaveBeenCalled()
      expect(row().getByRole('button', { name: 'Play: AlanHowick' })).toBeInTheDocument()
    })

    it('should keep playing a clip the visitor started with the button', async () => {
      const media = setMedia(true)
      const user = userEvent.setup()
      render(
        <CatchTheVibe title="t" items={items} labels={labels} carouselLabels={carouselLabels} />
      )

      // From the keyboard, so no pointer preview starts first.
      row().getByRole('button', { name: 'Play: AlanHowick' }).focus()
      await user.keyboard('{Enter}')
      act(() => media.reduceMotion())
      fireEvent.mouseLeave(row().getByRole('listitem'))
      expect(row().getByRole('button', { name: 'Pause: AlanHowick' })).toBeInTheDocument()
    })
  })

  describe('when shown below the desktop breakpoint', () => {
    const clips = [
      ...items,
      { id: 'roustan', image: '/roustan.webp', video: '/roustan.mp4', creator: { name: 'Roustan' } }
    ]

    it('should offer the clips as a carousel, the current one interactive and the other an inert peek', () => {
      setMedia(false)
      render(
        <CatchTheVibe
          title="Catch the Vibe"
          items={clips}
          labels={labels}
          carouselLabels={carouselLabels}
        />
      )

      expect(screen.getByRole('group', { name: 'Catch the Vibe' })).toHaveAttribute(
        'aria-roledescription',
        'carousel'
      )
      expect(
        within(screen.getByRole('group', { name: 'Slide 1 of 2' })).getByRole('button', {
          name: 'Play: AlanHowick'
        })
      ).toBeInTheDocument()
      const track = screen.getByRole('group', { name: 'Slide 1 of 2' }).parentElement!
      const peeks = track.querySelectorAll('[inert]')
      expect([...peeks].every((peek) => peek.getAttribute('aria-hidden') === 'true')).toBe(true)
      expect([...peeks].filter((peek) => peek.childElementCount > 0)).toHaveLength(2)
    })

    it('should stop the playing clip when the visitor moves to the next one', async () => {
      setMedia(false)
      const user = userEvent.setup()
      render(
        <CatchTheVibe title="t" items={clips} labels={labels} carouselLabels={carouselLabels} />
      )

      const slide = () => screen.getByRole('group', { name: /^Slide \d of 2$/ })
      await user.click(within(slide()).getByRole('button', { name: 'Play: AlanHowick' }))
      expect(within(slide()).getByRole('button', { name: 'Pause: AlanHowick' })).toBeInTheDocument()

      await user.click(screen.getByRole('button', { name: 'Next slide' }))
      expect(slide()).toHaveAccessibleName('Slide 2 of 2')
      expect(within(slide()).getByRole('button', { name: 'Play: Roustan' })).toHaveAttribute(
        'aria-pressed',
        'false'
      )
    })

    it('should keep keyboard focus on the current clip control as the arrow keys change clips', async () => {
      setMedia(false)
      const user = userEvent.setup()
      render(
        <CatchTheVibe title="t" items={clips} labels={labels} carouselLabels={carouselLabels} />
      )

      const slide = () => screen.getByRole('group', { name: /^Slide \d of 2$/ })
      within(slide()).getByRole('button', { name: 'Play: AlanHowick' }).focus()
      await user.keyboard('{ArrowRight}')
      expect(slide()).toHaveAccessibleName('Slide 2 of 2')
      expect(within(slide()).getByRole('button', { name: 'Play: Roustan' })).toHaveFocus()
      await user.keyboard('{ArrowLeft}')
      expect(slide()).toHaveAccessibleName('Slide 1 of 2')
      expect(within(slide()).getByRole('button', { name: 'Play: AlanHowick' })).toHaveFocus()
      await user.keyboard('{Enter}')
      expect(within(slide()).getByRole('button', { name: 'Pause: AlanHowick' })).toHaveFocus()
    })
  })
})

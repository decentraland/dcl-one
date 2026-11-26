import type { ReactElement } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'

import { PlaceDetail } from './PlaceDetail'

type Props = Parameters<typeof PlaceDetail>[0]

const place = {
  id: 'p1',
  title: 'Meta GamiMall',
  coords: '1,95',
  description: 'Shops and games',
  image: 'https://example.test/content/contents/bafkcover'
}
const creator = { name: 'MetaDoge', address: '0x797066a17f83425c1b4c7a8cca52d19095520a52' }
const JUMP = '/play/?position=1%2C95'
const mount = (over: Partial<Props> = {}) =>
  render(<PlaceDetail place={place} creator={creator} livePlayers={6} jumpHref={JUMP} {...over} />)
const FOOTER = 'Jump into Meta GamiMall in Decentraland to participate in the chat.'
const footer = () => screen.getByText(/in Decentraland to participate in the chat/).textContent

describe('PlaceDetail', () => {
  let container: HTMLElement

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when the cover address has a quote', () => {
    beforeEach(() => {
      ;({ container } = mount({
        place: { ...place, image: 'https://example.test/a.png");color:red;x:url("' }
      }))
    })

    it('should keep the quote inside the CSS string', () => {
      const hero = container.querySelector<HTMLElement>('[style*="--cover"]')
      expect(hero?.style.getPropertyValue('--cover')).toBe(
        'url("https://example.test/a.png\\");color:red;x:url(\\"")'
      )
    })
  })

  describe('when a desktop visitor opens a scene with players', () => {
    beforeEach(() => {
      mount()
    })

    it('should render the viewer with the place title and creator', () => {
      const viewer = screen.getByRole('region', { name: 'Meta GamiMall' })
      expect(
        within(viewer).getByRole('heading', { level: 1, name: 'Meta GamiMall' })
      ).toBeInTheDocument()
      expect(within(viewer).getByText('MetaDoge')).toBeInTheDocument()
    })

    it('should draw the bar actions with the shared Button and JumpInCTA', () => {
      const explore = screen.getByRole('button', { name: 'Explore the scene' })
      expect(explore).toHaveAttribute('data-variant', 'inverse')
      expect(explore).toHaveAttribute('data-size', 'md')
      expect(explore).toBeEnabled()
    })

    it('should link both jump-in actions to the scene', () => {
      expect(
        screen.getByRole('link', { name: 'Jump into 1,95 (opens in a new tab)' })
      ).toHaveAttribute('href', JUMP)
      expect(screen.getByRole('link', { name: 'Jump in' })).toHaveAttribute('href', JUMP)
    })

    it('should render the chat panel', () => {
      expect(screen.getByRole('heading', { name: 'In-World Chat' })).toBeInTheDocument()
      expect(screen.getByText('No messages yet.')).toBeInTheDocument()
    })

    it('should say what to expect', () => {
      expect(screen.getByText('What to expect')).toBeInTheDocument()
    })

    it('should not open the jump-in dialog', () => {
      expect(screen.queryByRole('dialog')).toBeNull()
    })

    describe('and the visitor explores the scene', () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole('button', { name: 'Explore the scene' }))
      })

      it('should open the explorer in the viewer', () => {
        expect(screen.getByTitle('Meta GamiMall')).toHaveAttribute('src', JUMP)
      })

      describe('and then stops', () => {
        beforeEach(() => {
          fireEvent.click(screen.getByRole('button', { name: 'Stop' }))
        })

        it('should close the explorer', () => {
          expect(screen.queryByTitle('Meta GamiMall')).toBeNull()
        })
      })
    })
  })

  describe('when the scene has players but no jump link yet', () => {
    let onJumpIn: Mock<() => void>

    beforeEach(() => {
      onJumpIn = vi.fn()
      mount({ jumpHref: undefined, onJumpIn })
    })

    it('should not let the visitor explore the scene', () => {
      expect(screen.getByRole('button', { name: 'Explore the scene' })).toBeDisabled()
    })

    it('should make Jump in a button that hands the press to the host', () => {
      expect(screen.queryByRole('link', { name: 'Jump in' })).toBeNull()
      fireEvent.click(screen.getByRole('button', { name: 'Jump in' }))
      expect(onJumpIn).toHaveBeenCalledOnce()
    })
  })

  describe("when the route's travel error is in the detail slot", () => {
    let slot: ReactElement

    beforeEach(() => {
      slot = (
        <div role="alert">
          <button type="button">Retry travel</button>
        </div>
      )
    })

    describe('and the scene has no players', () => {
      beforeEach(() => {
        mount({ livePlayers: 0, jumpHref: undefined, detailSlot: slot })
      })

      it('should place the retry in the dialog, above the backdrop', () => {
        expect(
          within(screen.getByRole('dialog')).getByRole('button', { name: 'Retry travel' })
        ).toBeInTheDocument()
      })
    })

    describe('and the visitor is on a phone', () => {
      beforeEach(() => {
        mount({ mobile: true, detailSlot: slot })
      })

      it('should place the error in the dialog, above the backdrop', () => {
        expect(within(screen.getByRole('dialog')).getByRole('alert')).toBeInTheDocument()
      })
    })
  })

  describe('when the chat panel goes through its states', () => {
    let rerender: (ui: ReactElement) => void
    let at: number

    const withChat = (chat: Props['chat']) => (
      <PlaceDetail place={place} livePlayers={6} jumpHref={JUMP} chat={chat} />
    )

    beforeEach(() => {
      at = new Date(2026, 0, 1, 9, 5).getTime()
      ;({ rerender } = render(withChat({ status: 'loading', messages: [] })))
    })

    describe('and the room is being asked for', () => {
      it('should show the empty chat', () => {
        expect(screen.getByText('No messages yet.')).toBeInTheDocument()
      })

      it('should invite the visitor to jump in to participate', () => {
        expect(footer()).toBe(FOOTER)
      })
    })

    describe('and there is no chat', () => {
      beforeEach(() => {
        rerender(withChat({ status: 'unavailable', messages: [] }))
      })

      it('should say the chat is unavailable', () => {
        expect(screen.getByText('Chat is unavailable right now.')).toBeInTheDocument()
      })

      describe('and the room then turns out silent', () => {
        beforeEach(() => {
          rerender(withChat({ status: 'ready', messages: [] }))
        })

        it('should show the empty chat', () => {
          expect(screen.getByText('No messages yet.')).toBeInTheDocument()
        })

        describe('and messages arrive', () => {
          beforeEach(() => {
            rerender(
              withChat({
                status: 'ready',
                messages: [
                  {
                    sender: 'brave-lynx',
                    name: 'brave-lynx',
                    avatarUrl: '/lynx.png',
                    avatarBackground: 'red',
                    timestamp: at,
                    message: 'hello from the cast'
                  },
                  {
                    sender: '0x797066a17F83425C1B4C7a8Cca52D19095520a52',
                    name: '…',
                    avatarUrl: '/wallet.png',
                    avatarBackground: 'white',
                    timestamp: at,
                    message: 'gm'
                  }
                ]
              })
            )
          })

          it('should drop the empty chat', () => {
            expect(screen.queryByText('No messages yet.')).toBeNull()
          })

          it('should show each sender and message', () => {
            expect(screen.getByText('brave-lynx')).toBeInTheDocument()
            expect(screen.getByText('hello from the cast')).toBeInTheDocument()
            expect(screen.getByText('gm')).toBeInTheDocument()
            expect(screen.getByText('…')).toBeInTheDocument()
          })

          it('should show the time of each message', () => {
            expect(
              screen.getAllByText(
                new Date(at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
              )
            ).toHaveLength(2)
          })

          it('should keep the same footer shell', () => {
            expect(footer()).toBe(FOOTER)
          })
        })
      })
    })
  })

  describe('when the world has a scene picker', () => {
    let onChange: Mock<(value: string) => void>
    let options: { value: string; label: string }[]

    beforeEach(() => {
      onChange = vi.fn()
      options = [
        { value: 'a', label: 'Plaza (0,0)' },
        { value: 'b', label: 'Arena (10,4)' }
      ]
    })

    describe('and it has more than one scene', () => {
      beforeEach(() => {
        mount({ scenePicker: { options, value: 'a', onChange } })
        fireEvent.change(screen.getByRole('combobox', { name: 'Scene' }), {
          target: { value: 'b' }
        })
      })

      it('should pick the scene to jump into', () => {
        expect(onChange).toHaveBeenCalledWith('b')
      })
    })

    describe('and it has one scene', () => {
      beforeEach(() => {
        mount({ scenePicker: { options: options.slice(0, 1), value: 'a', onChange } })
      })

      it('should not render a picker', () => {
        expect(screen.queryByRole('combobox')).toBeNull()
      })
    })
  })

  describe("when the creator's name is clicked", () => {
    let onCreator: Mock<(address: string) => void>

    beforeEach(() => {
      onCreator = vi.fn()
      mount({ onCreator })
      fireEvent.click(screen.getByRole('button', { name: 'MetaDoge' }))
    })

    it("should open the owner's profile", () => {
      expect(onCreator).toHaveBeenCalledWith('0x797066a17f83425c1b4c7a8cca52d19095520a52')
    })
  })

  describe('when the scene is empty', () => {
    let dialog: HTMLElement

    beforeEach(() => {
      mount({ livePlayers: 0 })
      dialog = screen.getByRole('dialog', { name: 'Meta GamiMall' })
    })

    it('should give the focus to the jump-in dialog', () => {
      expect(document.activeElement).toBe(within(dialog).getByRole('button', { name: 'Close' }))
    })

    it('should keep Tab inside the jump-in dialog', () => {
      const close = within(dialog).getByRole('button', { name: 'Close' })
      const controls = dialog.querySelectorAll<HTMLElement>('a[href], button:not([disabled])')
      const last = controls[controls.length - 1]!
      fireEvent.keyDown(close, { key: 'Tab', shiftKey: true })
      expect(document.activeElement).toBe(last)
      fireEvent.keyDown(last, { key: 'Tab' })
      expect(document.activeElement).toBe(close)
    })

    it('should show the jump-in card actions', () => {
      expect(within(dialog).getByRole('button', { name: 'Close' })).toBeInTheDocument()
      expect(within(dialog).getByRole('button', { name: 'Copy link' })).toBeInTheDocument()
      expect(within(dialog).getByRole('link', { name: 'Jump in' })).toHaveAttribute('href', JUMP)
    })

    it('should draw the close button and the copy button as shared controls', () => {
      expect(within(dialog).getByRole('button', { name: 'Close' })).toHaveAttribute(
        'data-variant',
        'filled'
      )
      const copy = within(dialog).getByRole('button', { name: 'Copy link' })
      expect(copy).toHaveAttribute('data-variant', 'outline')
      expect(copy).toHaveAttribute('data-icon-only', 'true')
      expect(copy).toHaveAttribute('data-size', 'lg')
    })

    it('should not show the live badges', () => {
      expect(screen.queryByTestId('SensorsRoundedIcon')).toBeNull()
      expect(screen.queryByText('In-World Chat')).toBeNull()
    })
  })

  describe('when the visitor closes the jump-in dialog', () => {
    let onClose: Mock<() => void>

    beforeEach(() => {
      onClose = vi.fn()
      mount({ livePlayers: 0, onClose })
      fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    })

    it('should ask the host to close it', () => {
      expect(onClose).toHaveBeenCalledOnce()
    })
  })

  describe('when the visitor copies the link from the jump-in dialog', () => {
    let writeText: Mock<(text: string) => Promise<void>>

    beforeEach(() => {
      writeText = vi
        .fn<(text: string) => Promise<void>>()
        .mockRejectedValueOnce(new Error('denied'))
        .mockResolvedValueOnce(undefined)
      Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
      mount({ livePlayers: 0 })
    })

    it('should stay silent when the clipboard refuses and confirm once it accepts', async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Copy link' }))
      await waitFor(() => expect(writeText).toHaveBeenCalledOnce())
      expect(screen.queryByText('Copied!')).toBeNull()
      fireEvent.click(screen.getByRole('button', { name: 'Copy link' }))
      expect(await screen.findByRole('status')).toHaveTextContent('Copied!')
    })
  })

  describe('when the visitor is on a phone', () => {
    beforeEach(() => {
      mount({ mobile: true })
    })

    it('should show the jump-in card', () => {
      expect(screen.getByRole('dialog')).toBeInTheDocument()
    })

    it('should show the live player count', () => {
      expect(screen.getByText('Live')).toBeInTheDocument()
      expect(screen.getByTestId('PermIdentityRoundedIcon').parentElement).toHaveTextContent('6')
    })
  })

  describe('when the place is missing', () => {
    beforeEach(() => {
      render(<PlaceDetail notFound />)
    })

    it('should say the scene was not found', () => {
      expect(screen.getByRole('heading', { name: 'Scene not found' })).toBeInTheDocument()
    })
  })
})

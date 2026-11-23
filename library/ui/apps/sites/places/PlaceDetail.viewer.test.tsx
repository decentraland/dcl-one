import type { ComponentProps } from 'react'
import {
  render as renderDom,
  screen,
  waitFor,
  within,
  type RenderResult
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { PlaceDetail } from './PlaceDetail'

type Props = ComponentProps<typeof PlaceDetail>
type Write = Mock<(id: string, value: boolean | null) => Promise<void>>
const place = {
  id: 'plaza',
  title: 'Genesis Plaza',
  coords: '-3,-2',
  description: '',
  approval: 87,
  favorites: 1135
}
const saved = () => Promise.resolve()
const refused = () => Promise.reject(new Error('Sign in to rate or save places.'))

const page = (over: Partial<Props> = {}) => (
  <PlaceDetail place={place} livePlayers={3} onLike={saved} onFavorite={saved} {...over} />
)
const mount = (over: Partial<Props> = {}) => renderDom(page(over))
const button = (name: string) => screen.getByRole('button', { name })
const pressed = () =>
  ['Like', 'Dislike', 'Favorite'].map((name) => button(name).getAttribute('aria-pressed'))

describe('PlaceDetail rating and saving', () => {
  let user: ReturnType<typeof userEvent.setup>
  let onLike: Write
  let onFavorite: Write
  let rerender: RenderResult['rerender']

  beforeEach(() => {
    user = userEvent.setup()
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when the place is shown in the live view', () => {
    let live: HTMLElement

    beforeEach(() => {
      mount()
      live = screen.getByRole('group', { name: 'Rate and save this place' })
    })

    it('should carry the three buttons', () => {
      expect(
        within(live)
          .getAllByRole('button')
          .map((b) => b.getAttribute('aria-label'))
      ).toEqual(['Like', 'Dislike', 'Favorite'])
    })

    it("should show the place's approval", () => {
      expect(live).toHaveTextContent('87% likes')
    })

    it("should show the place's favorites", () => {
      expect(live).toHaveTextContent('1,135 favorites')
    })
  })

  describe('when the place is shown on the jump-in card', () => {
    let card: HTMLElement

    beforeEach(() => {
      mount({ livePlayers: 0 })
      card = within(screen.getByRole('dialog', { name: 'Genesis Plaza' })).getByRole('group', {
        name: 'Rate and save this place'
      })
    })

    it('should carry the three buttons', () => {
      expect(within(card).getAllByRole('button')).toHaveLength(3)
    })

    it("should show the place's favorites", () => {
      expect(card).toHaveTextContent('1,135 favorites')
    })
  })

  describe('when nobody rated the place and one visitor saved it', () => {
    let group: HTMLElement

    beforeEach(() => {
      mount({ place: { ...place, approval: null, favorites: 1 } })
      group = screen.getByRole('group', { name: 'Rate and save this place' })
    })

    it('should show no approval', () => {
      expect(group).not.toHaveTextContent('%')
    })

    it('should show a singular favorite', () => {
      expect(group).toHaveTextContent(/1 favorite$/)
    })
  })

  describe('when the host provides no writes', () => {
    beforeEach(() => {
      mount({ onLike: undefined, onFavorite: undefined })
    })

    it('should render no buttons', () => {
      expect(screen.queryByRole('button', { name: 'Like' })).toBeNull()
    })

    it('should render no rating group', () => {
      expect(screen.queryByRole('group', { name: 'Rate and save this place' })).toBeNull()
    })
  })

  describe('when the visitor likes the place', () => {
    beforeEach(async () => {
      onLike = vi.fn(saved)
      mount({ onLike })
      await user.click(button('Like'))
    })

    it('should save the like', () => {
      expect(onLike).toHaveBeenLastCalledWith('plaza', true)
    })

    it('should press only Like', () => {
      expect(pressed()).toEqual(['true', 'false', 'false'])
    })

    describe('and then dislikes it', () => {
      beforeEach(async () => {
        await user.click(button('Dislike'))
      })

      it('should save the dislike', () => {
        expect(onLike).toHaveBeenLastCalledWith('plaza', false)
      })

      it('should press only Dislike', () => {
        expect(pressed()).toEqual(['false', 'true', 'false'])
      })

      describe('and presses Dislike a second time', () => {
        beforeEach(async () => {
          await user.click(button('Dislike'))
        })

        it('should clear the saved choice', () => {
          expect(onLike).toHaveBeenLastCalledWith('plaza', null)
        })

        it('should press nothing', () => {
          expect(pressed()).toEqual(['false', 'false', 'false'])
        })
      })
    })
  })

  describe('when the visitor favorites the place', () => {
    beforeEach(async () => {
      onFavorite = vi.fn(saved)
      mount({ onFavorite })
      await user.click(button('Favorite'))
    })

    it('should save the favorite', () => {
      expect(onFavorite).toHaveBeenLastCalledWith('plaza', true)
    })

    it('should press only Favorite', () => {
      expect(pressed()).toEqual(['false', 'false', 'true'])
    })

    describe('and presses Favorite a second time', () => {
      beforeEach(async () => {
        await user.click(button('Favorite'))
      })

      it('should remove the save', () => {
        expect(onFavorite).toHaveBeenLastCalledWith('plaza', false)
      })

      it('should press nothing', () => {
        expect(pressed()).toEqual(['false', 'false', 'false'])
      })
    })
  })

  describe('when the visitor is signed out and the host refuses every write', () => {
    beforeEach(() => {
      onLike = vi.fn(refused)
      onFavorite = vi.fn(refused)
      mount({ onLike, onFavorite })
    })

    it('should put each pressed button back and send each write once', async () => {
      for (const name of ['Like', 'Dislike', 'Favorite']) {
        await user.click(button(name))
        await waitFor(() => expect(pressed()).toEqual(['false', 'false', 'false']))
      }
      expect(onLike).toHaveBeenCalledTimes(2)
      expect(onFavorite).toHaveBeenCalledTimes(1)
    })
  })

  describe('when a write fails', () => {
    let fail: (reason: Error) => void

    beforeEach(() => {
      onLike = vi.fn(
        () =>
          new Promise<void>((_, reject) => {
            fail = reject
          })
      )
      onFavorite = vi.fn(() => Promise.reject(new Error('503')))
      mount({ viewer: { liked: true, favorite: true }, account: '0xa', onLike, onFavorite })
    })

    it("should put the button back to the account's saved state", async () => {
      await user.click(button('Dislike'))
      // The press shows while the write is on its way.
      expect(pressed()).toEqual(['false', 'true', 'true'])
      fail(new Error('503'))
      await waitFor(() => expect(pressed()).toEqual(['true', 'false', 'true']))
      await user.click(button('Favorite'))
      expect(onFavorite).toHaveBeenLastCalledWith('plaza', false)
      await waitFor(() => expect(pressed()).toEqual(['true', 'false', 'true']))
    })
  })

  describe("when the account's saved state is known only after the first render", () => {
    beforeEach(() => {
      ;({ rerender } = mount({ account: '0xa' }))
    })

    it('should press nothing before it arrives', () => {
      expect(pressed()).toEqual(['false', 'false', 'false'])
    })

    describe('and the account liked and saved the place', () => {
      beforeEach(() => {
        rerender(page({ account: '0xa', viewer: { liked: true, favorite: true } }))
      })

      it('should press Like and Favorite', () => {
        expect(pressed()).toEqual(['true', 'false', 'true'])
      })

      describe('and the account then disliked and unsaved it', () => {
        beforeEach(() => {
          rerender(page({ account: '0xa', viewer: { liked: false, favorite: false } }))
        })

        it('should press only Dislike', () => {
          expect(pressed()).toEqual(['false', 'true', 'false'])
        })
      })
    })
  })

  describe('when the visitor pressed a button before the saved state arrived', () => {
    beforeEach(async () => {
      onLike = vi.fn(saved)
      ;({ rerender } = mount({ account: '0xa', onLike }))
      await user.click(button('Favorite'))
      rerender(page({ account: '0xa', onLike, viewer: { liked: true, favorite: false } }))
    })

    it("should keep the visitor's choice and let the others follow the account", () => {
      expect(pressed()).toEqual(['true', 'false', 'true'])
    })

    it('should not write the buttons that followed the account', () => {
      expect(onLike).not.toHaveBeenCalled()
    })
  })

  describe('when the page changes from the live view to the jump-in card after a press', () => {
    beforeEach(async () => {
      ;({ rerender } = mount())
      await user.click(button('Like'))
      rerender(page({ livePlayers: 0 }))
    })

    it('should show the jump-in card', () => {
      expect(screen.getByRole('dialog')).toBeInTheDocument()
    })

    it('should keep the press', () => {
      expect(pressed()).toEqual(['true', 'false', 'false'])
    })
  })

  describe('when the visitor made choices under one account', () => {
    beforeEach(async () => {
      ;({ rerender } = mount({ account: '0xa', viewer: { liked: null, favorite: false } }))
      await user.click(button('Like'))
      await user.click(button('Favorite'))
    })

    it('should press the chosen buttons', () => {
      expect(pressed()).toEqual(['true', 'false', 'true'])
    })

    describe('and the visitor signs out', () => {
      beforeEach(() => {
        rerender(page({ account: null, viewer: null }))
      })

      it('should forget the choices', () => {
        expect(pressed()).toEqual(['false', 'false', 'false'])
      })

      describe('and another account signs in', () => {
        beforeEach(() => {
          rerender(page({ account: '0xb', viewer: { liked: false, favorite: false } }))
        })

        it("should follow the new account's saved state", () => {
          expect(pressed()).toEqual(['false', 'true', 'false'])
        })
      })
    })
  })
})

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  CommunityFrame,
  CommunityInfoCard,
  CommunityMembershipAction,
  CommunitySections,
  type CommunityMembershipActionProps,
  type CommunityMembershipState
} from './CommunityDetail'
import { CommunityMembersList, type CommunityListMore } from './CommunityLists'

afterEach(cleanup)

const more: CommunityListMore = {
  state: 'none',
  manual: false,
  sentinelRef: null,
  onLoadMore: () => {}
}

const ownerFace = (container: HTMLElement) => container.querySelector('p img')

describe('CommunityInfoCard', () => {
  let container: HTMLElement

  describe("when the owner's face loads", () => {
    beforeEach(() => {
      ;({ container } = render(
        <CommunityInfoCard
          name="Decentraland Foundation"
          privacy="public"
          membersLabel="1.3K Members"
          thumbnailSrc="/assets-cdn/social/communities/id/raw-thumbnail.png"
          owner={{ name: 'DCLOfficial', face: '/profile-images/entities/x/face.png' }}
          action={null}
        />
      ))
    })

    it('should show the face', () => {
      expect(ownerFace(container)?.getAttribute('src')).toBe('/profile-images/entities/x/face.png')
    })

    describe('and the face fails to load', () => {
      beforeEach(() => {
        fireEvent.error(ownerFace(container)!)
      })

      it("should show the owner's initial instead", () => {
        expect(ownerFace(container)).toBeNull()
        expect(screen.getByText('D')).toBeTruthy()
      })
    })
  })

  describe('when the owner has no face', () => {
    beforeEach(() => {
      ;({ container } = render(
        <CommunityInfoCard
          name="Decentraland Foundation"
          privacy="private"
          membersLabel="3 Members"
          thumbnailSrc="/assets-cdn/social/communities/id/raw-thumbnail.png"
          owner={{ name: 'zed' }}
          action={null}
          description={null}
        />
      ))
    })

    it("should show the owner's initial, the privacy and no description", () => {
      expect(ownerFace(container)).toBeNull()
      expect(screen.getByText('Z')).toBeTruthy()
      expect(screen.getByText('Private')).toBeTruthy()
      expect(container.querySelectorAll('section > p')).toHaveLength(0)
    })
  })
})

describe('CommunityMembersList', () => {
  afterEach(() => {
    vi.resetAllMocks()
  })

  describe("when a member's face fails to load", () => {
    beforeEach(() => {
      render(
        <CommunityMembersList
          total={1}
          status="ready"
          members={[
            {
              address: '0x1',
              name: 'alice',
              face: '/profile-images/entities/a/face.png',
              role: 'Owner',
              claimedName: true
            }
          ]}
          onRetry={() => {}}
          more={more}
        />
      )
      fireEvent.error(screen.getByRole('listitem').querySelector('img')!)
    })

    it("should show the member's initial instead of a broken image", () => {
      expect(screen.getByRole('listitem').querySelector('img')).toBeNull()
      expect(screen.getByText('A')).toBeTruthy()
      expect(screen.getByRole('img', { name: 'Claimed name' })).toBeTruthy()
    })
  })

  describe('when the next page fails', () => {
    let onLoadMore: ReturnType<typeof vi.fn>

    beforeEach(() => {
      onLoadMore = vi.fn()
      render(
        <CommunityMembersList
          total={20}
          status="error"
          members={[{ address: '0x1', name: 'alice', role: 'Member', claimedName: false }]}
          onRetry={() => {}}
          more={{ ...more, state: 'error', onLoadMore }}
        />
      )
      fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    })

    it('should keep the members shown and read the next page again on Try again', () => {
      expect(screen.getByText('alice')).toBeTruthy()
      expect(screen.getByText("We couldn't load more members.")).toBeTruthy()
      expect(onLoadMore).toHaveBeenCalledTimes(1)
    })
  })
})

describe('CommunityFrame', () => {
  describe('when a dialog is open', () => {
    let container: HTMLElement

    beforeEach(() => {
      ;({ container } = render(
        <CommunityFrame dialog={<div role="dialog" aria-label="Event" />}>
          <h1>Decentraland Foundation</h1>
        </CommunityFrame>
      ))
    })

    it("should render it outside the page's stacking context", () => {
      const page = Array.from(container.children).find((child) =>
        child.contains(screen.getByRole('heading'))
      )!
      expect(page.className).toContain('root')
      expect(page.contains(screen.getByRole('dialog'))).toBe(false)
    })
  })

  describe('when no dialog is open', () => {
    it('should render only the page', () => {
      const { container } = render(
        <CommunityFrame>
          <h1>Decentraland Foundation</h1>
        </CommunityFrame>
      )
      expect(container.children).toHaveLength(1)
    })
  })
})

describe('CommunityMembershipAction', () => {
  let props: CommunityMembershipActionProps

  const show = (state: CommunityMembershipState, pending = false) => {
    props = {
      state,
      pending,
      onSignIn: vi.fn(),
      onRetry: vi.fn(),
      onJoin: vi.fn(),
      onRequest: vi.fn(),
      onCancel: vi.fn()
    }
    render(<CommunityMembershipAction {...props} />)
  }

  describe.each([
    ['signed-out', 'Sign in to join', 'onSignIn'],
    ['join', 'Join', 'onJoin'],
    ['request', 'Request to join', 'onRequest'],
    ['cancel', 'Cancel request', 'onCancel'],
    ['error', 'Try again', 'onRetry']
  ] as const)('when the state is %s', (state, label, handler) => {
    beforeEach(() => {
      show(state)
      fireEvent.click(screen.getByRole('button', { name: label }))
    })

    it(`should offer ${label} and run only its action`, () => {
      expect(props[handler]).toHaveBeenCalledTimes(1)
      const others = (['onSignIn', 'onRetry', 'onJoin', 'onRequest', 'onCancel'] as const).filter(
        (name) => name !== handler
      )
      for (const name of others) expect(props[name]).not.toHaveBeenCalled()
    })
  })

  describe('when the membership could not be checked', () => {
    beforeEach(() => {
      show('error')
    })

    it('should say so', () => {
      expect(screen.getByRole('alert').textContent).toBe(
        "We couldn't check your membership right now."
      )
    })
  })

  describe('when the visitor is a member', () => {
    beforeEach(() => {
      show('joined')
    })

    it('should show Joined, disabled', () => {
      expect(screen.getByRole('button', { name: 'Joined' })).toHaveProperty('disabled', true)
    })
  })

  describe('when the membership is loading', () => {
    beforeEach(() => {
      show('loading')
    })

    it('should offer nothing to press', () => {
      expect(screen.getByRole('button')).toHaveProperty('disabled', true)
    })
  })

  describe.each([
    ['join', 'Join'],
    ['request', 'Request to join'],
    ['cancel', 'Cancel request']
  ] as const)('when a %s is on its way', (state, label) => {
    beforeEach(() => {
      show(state, true)
    })

    it(`should disable ${label} so it is not sent twice`, () => {
      expect(screen.getByRole('button', { name: label })).toHaveProperty('disabled', true)
    })
  })
})

describe('CommunitySections', () => {
  beforeEach(() => {
    render(<CommunitySections members={<p>Alice</p>} events={<p>Takeover Tuesday</p>} />)
  })

  const panelOf = (text: string) => screen.getByText(text).closest('[role="tabpanel"]')!

  describe('when it first shows', () => {
    it('should select Members and keep both sections in the page', () => {
      expect(screen.getByRole('tab', { name: 'Members', selected: true })).toBeTruthy()
      expect(panelOf('Alice').className).not.toContain('inactive')
      expect(panelOf('Takeover Tuesday').className).toContain('inactive')
    })

    it('should label each panel by its tab', () => {
      const tab = screen.getByRole('tab', { name: 'Upcoming Events' })
      expect(tab.getAttribute('aria-controls')).toBe(panelOf('Takeover Tuesday').id)
      expect(panelOf('Takeover Tuesday').getAttribute('aria-labelledby')).toBe(tab.id)
    })
  })

  describe('when the visitor picks Upcoming Events', () => {
    beforeEach(() => {
      fireEvent.click(screen.getByRole('tab', { name: 'Upcoming Events' }))
    })

    it('should show the events instead of the members', () => {
      expect(screen.getByRole('tab', { name: 'Upcoming Events', selected: true })).toBeTruthy()
      expect(panelOf('Takeover Tuesday').className).not.toContain('inactive')
      expect(panelOf('Alice').className).toContain('inactive')
    })
  })
})

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CommunityInfoCard } from './CommunityDetail'
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

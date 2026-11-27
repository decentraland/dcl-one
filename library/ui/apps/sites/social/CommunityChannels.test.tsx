import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  CommunityDirectory,
  CommunityHangouts,
  CommunityLockedChannel,
  CommunityOverview,
  type CommunityOverviewProps
} from './CommunityChannels'

afterEach(cleanup)

const NOW = Date.parse('2026-09-21T12:00:00Z')
const author = '0x1111111111111111111111111111111111111111'
const club = {
  id: 'club',
  name: 'Music Club',
  description: 'Live sets every Friday.',
  membersCount: 4,
  privacy: 'public' as const,
  role: 'member'
}
const plaza = { id: 'plaza', title: 'Club Plaza', addedBy: author, addedAt: '2026-09-02T12:00:00Z' }

function overview(props: Partial<CommunityOverviewProps> = {}) {
  const all: CommunityOverviewProps = {
    community: club,
    joined: true,
    membershipAction: <button type="button">Joined</button>,
    link: 'https://decentraland.zone/social?community=club',
    postsLocked: false,
    latestPost: {
      id: 'p',
      authorAddress: author,
      authorName: 'Nyx',
      content: 'Newest word',
      createdAt: '2026-09-18T17:40:00Z',
      likesCount: 0
    },
    latestPending: false,
    showPlaces: true,
    places: [plaza],
    placesPending: false,
    onAnnouncements: vi.fn(),
    onHangouts: vi.fn(),
    now: NOW,
    ...props
  }
  return { props: all, ...render(<CommunityOverview {...all} />) }
}

describe('when a community opens on its overview', () => {
  it('should show its words, facts, latest announcement and places', () => {
    overview()
    expect(screen.getByRole('heading', { level: 1, name: 'Music Club' })).toBeTruthy()
    expect(screen.getByText('Live sets every Friday.')).toBeTruthy()
    expect(screen.getByText('Newest word')).toBeTruthy()
    expect(screen.getByText('Club Plaza')).toBeTruthy()
    const facts = screen.getByText('Your role').closest('div') as HTMLElement
    expect(facts).toHaveTextContent('Member')
  })

  it('should take the reader to the announcements and hangouts channels', async () => {
    const user = userEvent.setup()
    const { props } = overview()
    await user.click(screen.getByRole('button', { name: 'All announcements' }))
    await user.click(screen.getByRole('button', { name: /Club Plaza/ }))
    expect(props.onAnnouncements).toHaveBeenCalledTimes(1)
    expect(props.onHangouts).toHaveBeenCalledTimes(1)
  })

  it('should explain that a private community keeps its announcements for members', () => {
    overview({
      community: { ...club, privacy: 'private', role: null },
      joined: false,
      postsLocked: true,
      latestPost: null
    })
    expect(screen.getByText('Join this community to read its announcements.')).toBeTruthy()
    expect(screen.queryByText('No announcements yet.')).toBeNull()
    expect(screen.getByText('Not a member')).toBeTruthy()
  })

  it('should leave the places out when the community shares none', () => {
    overview({ showPlaces: false, places: [] })
    expect(screen.queryByRole('heading', { name: 'Its places' })).toBeNull()
  })

  it('should show why the last membership change failed', () => {
    overview({ membershipError: 'Your membership could not be changed. Try again.' })
    expect(screen.getByRole('alert')).toHaveTextContent('Your membership could not be changed.')
  })

  it('should name an unlisted community as such', () => {
    overview({ community: { ...club, visibility: 'unlisted' } })
    expect(screen.getByText('Unlisted')).toBeTruthy()
  })
})

describe('when the reader opens the members', () => {
  const members = [
    { memberAddress: author, name: 'Nyx', role: 'owner', hasClaimedName: true },
    { memberAddress: '0x2222222222222222222222222222222222222222', name: 'Ray', role: 'member' }
  ]

  it('should mark the reader and give the others the managers’ actions', () => {
    render(
      <CommunityDirectory
        name="Music Club"
        members={members}
        pending={false}
        viewer={author}
        actions={(member) => <button type="button">Manage {member.name}</button>}
      />
    )
    const self = screen.getByRole('listitem', { name: `Nyx ${author}` })
    expect(within(self).getByText('You')).toBeTruthy()
    expect(within(self).getByLabelText('verified')).toBeTruthy()
    expect(within(self).queryByRole('button')).toBeNull()
    expect(screen.getByRole('button', { name: 'Manage Ray' })).toBeTruthy()
  })

  it('should find members by name or address and say when nobody matches', async () => {
    const user = userEvent.setup()
    render(<CommunityDirectory name="Music Club" members={members} pending={false} viewer={null} />)
    const search = screen.getByRole('textbox', { name: 'Search members' })
    await user.type(search, '0x2222')
    expect(screen.getAllByRole('listitem')).toHaveLength(1)
    await user.clear(search)
    await user.type(search, 'nobody')
    expect(screen.getByText('No members match your search.')).toBeTruthy()
  })
})

describe('when the reader opens the hangouts', () => {
  it('should list each place with who added it and when', () => {
    render(<CommunityHangouts name="Music Club" places={[plaza]} pending={false} now={NOW} />)
    expect(screen.getByText('Places shared by Music Club.')).toBeTruthy()
    expect(screen.getByText(/Added by 0x1111…1111/)).toBeTruthy()
  })

  it('should say when nothing is shared', () => {
    render(<CommunityHangouts name="Music Club" places={[]} pending={false} />)
    expect(screen.getByText('No places shared yet.')).toBeTruthy()
  })
})

describe('when a channel is closed to the reader or the community cannot open', () => {
  it('should ask the reader to join to see a private channel', () => {
    render(<CommunityLockedChannel channel="Members" />)
    expect(screen.getByText('Join this community to view its members.')).toBeTruthy()
  })
})

describe('when the places API names no one who shared a place', () => {
  it('should show where it is and leave the sharer out', () => {
    render(
      <CommunityHangouts
        name="Music Club"
        places={[
          { id: 'stage', title: 'Main Stage', location: '-66,-56', addedBy: null, addedAt: null }
        ]}
        pending={false}
      />
    )
    expect(screen.getByText('-66,-56')).toBeTruthy()
    expect(screen.queryByText(/Added by/)).toBeNull()
  })
})

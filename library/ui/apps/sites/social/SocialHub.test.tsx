import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  HubConfirm,
  SocialExplore,
  SocialHome,
  SocialPanel,
  SocialRequests,
  type HubCommunityView,
  type HubRequestView,
  type SocialExploreProps,
  type SocialHomeProps,
  type SocialRequestsProps
} from './SocialHub'

afterEach(cleanup)

const club: HubCommunityView = {
  id: 'club',
  name: 'Music Club',
  description: 'Live sets',
  membersCount: 4,
  privacy: 'public',
  role: 'member'
}
const river: HubCommunityView = {
  id: 'river',
  name: 'River explorers',
  membersCount: 1,
  privacy: 'private',
  role: null
}
const href = (id: string) => `/social?community=${id}`

describe('when the reader opens the hub', () => {
  const home = (props: Partial<SocialHomeProps> = {}) => {
    const all: SocialHomeProps = {
      mine: [club],
      communities: [club, river],
      pending: false,
      failed: false,
      onRetry: vi.fn(),
      requestsCount: 0,
      onRequests: vi.fn(),
      onExplore: vi.fn(),
      onCreate: vi.fn(),
      communityHref: href,
      ...props
    }
    return { props: all, ...render(<SocialHome {...all} />) }
  }

  it('should list the reader’s communities as links to each', () => {
    home()
    expect(screen.getByRole('heading', { name: 'Your communities' })).toBeTruthy()
    expect(screen.getByRole('link', { name: /Music Club/ }).getAttribute('href')).toBe(
      '/social?community=club'
    )
    expect(screen.queryByRole('link', { name: /River explorers/ })).toBeNull()
  })

  it('should suggest communities to discover when the reader has none', () => {
    home({ mine: [] })
    expect(screen.getByRole('heading', { name: 'Discover communities' })).toBeTruthy()
    expect(screen.getByRole('link', { name: /River explorers/ })).toHaveTextContent(
      '1 member · Private'
    )
  })

  it('should offer invitations only while some are pending', async () => {
    const user = userEvent.setup()
    const { props, rerender } = home()
    expect(screen.queryByRole('button', { name: /Invitations/ })).toBeNull()
    rerender(<SocialHome {...props} requestsCount={2} />)
    await user.click(screen.getByRole('button', { name: 'Invitations (2)' }))
    expect(props.onRequests).toHaveBeenCalledTimes(1)
  })

  it('should retry a failed list and say when there is nothing yet', async () => {
    const user = userEvent.setup()
    const { props, rerender } = home({ mine: [], communities: [], failed: true })
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(props.onRetry).toHaveBeenCalledTimes(1)
    rerender(<SocialHome {...props} failed={false} />)
    expect(screen.getByText('No communities yet')).toBeTruthy()
  })

  it('should start a community', async () => {
    const user = userEvent.setup()
    const { props } = home()
    await user.click(screen.getByRole('button', { name: 'Create a community' }))
    expect(props.onCreate).toHaveBeenCalledTimes(1)
  })
})

describe('when the reader explores communities', () => {
  const explore = (props: Partial<SocialExploreProps> = {}) => {
    const all: SocialExploreProps = {
      mine: [club],
      communities: [club, river],
      query: '',
      pending: false,
      failed: false,
      onRetry: vi.fn(),
      onLoadMore: vi.fn(),
      loadingMore: false,
      communityHref: href,
      ...props
    }
    return { props: all, ...render(<SocialExplore {...all} />) }
  }

  it('should show the reader’s own first, then every community, joined ones marked', () => {
    explore()
    const mine = screen.getByRole('region', { name: 'Your communities' })
    expect(within(mine).getByRole('link', { name: /Music Club/ })).toBeTruthy()
    const all = screen.getByRole('region', { name: 'Explore communities' })
    expect(within(all).getByText('4 members · Joined')).toBeTruthy()
    expect(within(all).getByText('1 member')).toBeTruthy()
  })

  it('should hide the reader’s own while they search, and say when nothing matches', () => {
    const { props, rerender } = explore({ query: 'hiking', communities: [river] })
    expect(screen.queryByRole('region', { name: 'Your communities' })).toBeNull()
    rerender(<SocialExplore {...props} communities={[]} onLoadMore={undefined} />)
    expect(screen.getByText('No communities match your search.')).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Load more communities' })).toBeNull()
  })

  it('should load more on request and show that it is loading', async () => {
    const user = userEvent.setup()
    const { props, rerender } = explore()
    await user.click(screen.getByRole('button', { name: 'Load more communities' }))
    expect(props.onLoadMore).toHaveBeenCalledTimes(1)
    rerender(<SocialExplore {...props} loadingMore />)
    expect(screen.getByRole('button', { name: /Loading communities/ })).toBeDisabled()
  })

  it('should show loading cards and no stale results while the list loads', () => {
    explore({ pending: true })
    expect(screen.queryByText('River explorers')).toBeNull()
    expect(screen.getAllByRole('progressbar', { name: 'Loading communities' })).toHaveLength(6)
    expect(screen.queryByText('No communities match your search.')).toBeNull()
  })
})

describe('when the reader answers invitations and requests', () => {
  const invite: HubRequestView = {
    id: 'r1',
    communityId: 'club',
    memberAddress: '0xabc',
    type: 'invite',
    name: 'Music Club'
  }
  const asked: HubRequestView = {
    id: 'r2',
    communityId: 'river',
    memberAddress: '0xabc',
    type: 'request_to_join',
    name: 'River explorers'
  }
  const requests = (props: Partial<SocialRequestsProps> = {}) => {
    const all: SocialRequestsProps = {
      rows: [invite, asked],
      pending: false,
      busy: false,
      onRetry: vi.fn(),
      onDecide: vi.fn(),
      onOpen: vi.fn(),
      ...props
    }
    return { props: all, ...render(<SocialRequests {...all} />) }
  }

  it('should accept or decline an invitation and withdraw a request', async () => {
    const user = userEvent.setup()
    const { props } = requests()
    const invitation = screen.getByRole('listitem', { name: 'Music Club: invitation' })
    await user.click(within(invitation).getByRole('button', { name: 'Accept invitation' }))
    await user.click(
      within(screen.getByRole('listitem', { name: 'River explorers: join request' })).getByRole(
        'button',
        { name: 'Cancel request' }
      )
    )
    await user.click(within(invitation).getByRole('button', { name: 'Open community' }))
    expect(props.onDecide).toHaveBeenNthCalledWith(1, invite, 'accepted')
    expect(props.onDecide).toHaveBeenNthCalledWith(2, asked, 'cancelled')
    expect(props.onOpen).toHaveBeenCalledWith('club')
  })

  it('should wait while an answer is on its way', () => {
    requests({ busy: true })
    expect(screen.getByRole('button', { name: 'Accept invitation' })).toBeDisabled()
  })

  it('should let a manager approve requests and invite only a valid wallet', async () => {
    const user = userEvent.setup()
    const onInvite = vi.fn()
    const { props } = requests({
      managing: true,
      rows: [{ ...asked, name: null, memberAddress: '0x2222222222222222222222222222222222222222' }],
      onInvite
    })
    expect(screen.getByRole('heading', { name: 'Membership requests' })).toBeTruthy()
    await user.click(screen.getByRole('button', { name: 'Approve' }))
    expect(props.onDecide).toHaveBeenCalledWith(expect.objectContaining({ id: 'r2' }), 'accepted')
    const field = screen.getByRole('textbox', { name: 'Member wallet address' })
    await user.type(field, '0x123')
    expect(screen.getByRole('button', { name: 'Send invitation' })).toBeDisabled()
    await user.clear(field)
    await user.type(field, ' 0x3333333333333333333333333333333333333333 ')
    await user.click(screen.getByRole('button', { name: 'Send invitation' }))
    expect(onInvite).toHaveBeenCalledWith('0x3333333333333333333333333333333333333333')
  })

  it('should show a failure with its retry and nothing pending', async () => {
    const user = userEvent.setup()
    const { props } = requests({ rows: [], error: 'Membership requests could not be loaded.' })
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(props.onRetry).toHaveBeenCalledTimes(1)
    expect(screen.queryByText('No pending invitations or requests.')).toBeNull()
  })
})

describe('when the hub panel has a search', () => {
  it('should report what the reader types and clear it', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    const { rerender } = render(
      <SocialPanel
        label="Explore communities"
        heading="Explore communities"
        icon={null}
        search={{ label: 'Find a community', value: '', onChange }}
      >
        body
      </SocialPanel>
    )
    expect(screen.getByRole('region', { name: 'Explore communities' })).toBeTruthy()
    await user.type(screen.getByRole('textbox', { name: 'Find a community' }), 'h')
    expect(onChange).toHaveBeenLastCalledWith('h')
    rerender(
      <SocialPanel
        label="Explore communities"
        heading="Explore communities"
        icon={null}
        search={{ label: 'Find a community', value: 'hiking', onChange }}
      >
        body
      </SocialPanel>
    )
    await user.click(screen.getByRole('button', { name: 'Clear search' }))
    expect(onChange).toHaveBeenLastCalledWith('')
  })
})

describe('when a change needs confirmation', () => {
  it('should keep the dialog and its controls waiting while the change runs', async () => {
    const onCancel = vi.fn()
    render(
      <HubConfirm
        title="Leave community?"
        body="Are you sure?"
        confirmLabel="Leave community"
        busy
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />
    )
    const dialog = screen.getByRole('dialog', { name: 'Leave community?' })
    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toBeDisabled()
    expect(within(dialog).getByRole('button', { name: /Leave community/ })).toBeDisabled()
  })
})

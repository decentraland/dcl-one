import { describe, it, expect, afterEach } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import type { AppNotification } from '../../engine/protocol'
import { PopupHost, closeTopPopup, resetPopups } from '../../design'
import { clearRewardPopups, queueRewardPopups, rewardOf } from '../../features/rewards/RewardPopup'

const reward = (id: string, metadata: Record<string, unknown> = {}): AppNotification => ({
  id,
  type: 'reward_in_progress',
  timestamp: String(Date.now()),
  read: false,
  metadata: { title: 'New item', tokenName: `Hat ${id}`, tokenImage: `https://example.com/${id}.png`, tokenRarity: 'Epic', tokenCategory: 'hat', ...metadata }
})

const other = (id: string): AppNotification => ({ id, type: 'item_sold', timestamp: String(Date.now()), read: false, metadata: { title: `Sold ${id}` } })

const collect = (): void => act(() => screen.getByRole('button', { name: 'COLLECT' }).click())

afterEach(() => {
  act(() => {
    clearRewardPopups()
    resetPopups()
  })
})

describe('rewardOf', () => {
  it('reads the token fields a reward notification carries', () => {
    expect(rewardOf(reward('a'))).toEqual({ name: 'Hat a', image: 'https://example.com/a.png', rarity: 'epic', category: 'hat' })
  })

  it('lowercases the category like the rarity', () => {
    expect(rewardOf(reward('a', { tokenCategory: 'Upper_Body' }))?.category).toBe('upper_body')
  })

  it('falls back to base for an unknown or missing rarity', () => {
    expect(rewardOf(reward('a', { tokenRarity: 'shiny' }))?.rarity).toBe('base')
    expect(rewardOf(reward('a', { tokenRarity: undefined }))?.rarity).toBe('base')
  })

  it('ignores every other notification type', () => {
    expect(rewardOf(other('a'))).toBeNull()
  })
})

describe('reward popups', () => {
  it('opens one popup at a time, in the order given, and COLLECT shows the next', () => {
    render(<PopupHost />)
    act(() => queueRewardPopups([reward('a'), other('x'), reward('b')]))
    expect(screen.getByRole('dialog', { name: 'New Item Unlocked!' })).toBeTruthy()
    expect(screen.getByText('Hat a')).toBeTruthy()
    expect(screen.queryByText('Hat b')).toBeNull()
    collect()
    expect(screen.getByText('Hat b')).toBeTruthy()
    collect()
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('shows the rarity on the popup and the item image', () => {
    render(<PopupHost />)
    act(() => queueRewardPopups([reward('a')]))
    expect(screen.getByRole('dialog').getAttribute('data-rarity')).toBe('epic')
    expect(screen.getByRole('dialog').querySelector('img')?.getAttribute('src')).toBe('https://example.com/a.png')
  })

  it('the central Cancel closes it like COLLECT', () => {
    render(<PopupHost />)
    act(() => queueRewardPopups([reward('a'), reward('b')]))
    act(() => closeTopPopup())
    expect(screen.getByText('Hat b')).toBeTruthy()
  })

  it('opens nothing for a list without rewards', () => {
    render(<PopupHost />)
    act(() => queueRewardPopups([other('a')]))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('clear closes the popup on screen and drops the queue', () => {
    render(<PopupHost />)
    act(() => queueRewardPopups([reward('a'), reward('b')]))
    act(() => clearRewardPopups())
    expect(screen.queryByRole('dialog')).toBeNull()
    act(() => queueRewardPopups([reward('c')]))
    expect(screen.getByText('Hat c')).toBeTruthy()
  })
})

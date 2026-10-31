import { describe, it, expect, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BackpackPage } from '../features/backpack/BackpackPage'
import type { OutfitHistory } from '../features/session/useEngineSession'
import { enterAsGuest, fakeProfileState, fakeSession, renderSession } from './harness'

const BASE_MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'

function renderBar(history: Partial<OutfitHistory> = {}, opts: { initialTab?: 'emotes'; bodyShape?: string | null } = {}) {
  const s = fakeSession()
  const backpack = {
    ...s.backpack,
    open: true,
    bodyShape: opts.bodyShape === null ? undefined : (opts.bodyShape ?? BASE_MALE),
    outfitHistory: { canUndo: false, canRedo: false, rolling: false, ...history },
    randomizeOutfit: vi.fn(),
    undoOutfit: vi.fn(),
    redoOutfit: vi.fn()
  }
  render(<BackpackPage backpack={backpack} emotes={s.emotes} profile={fakeProfileState()} onNavigate={vi.fn()} setEngineViewport={vi.fn()} initialTab={opts.initialTab} />)
  return backpack
}

// The randomize bar under the avatar preview, like Unity's RandomizeOutfitView.
describe('backpack randomize bar', () => {
  it('rolls, undoes and redoes when each step is available', async () => {
    const backpack = renderBar({ canUndo: true, canRedo: true })
    await userEvent.click(screen.getByRole('button', { name: 'RANDOMIZE' }))
    await userEvent.click(screen.getByRole('button', { name: 'Undo outfit' }))
    await userEvent.click(screen.getByRole('button', { name: 'Redo outfit' }))
    expect(backpack.randomizeOutfit).toHaveBeenCalledTimes(1)
    expect(backpack.undoOutfit).toHaveBeenCalledTimes(1)
    expect(backpack.redoOutfit).toHaveBeenCalledTimes(1)
  })

  it('locks undo and redo without a step to take', () => {
    renderBar()
    expect(screen.getByRole('button', { name: 'RANDOMIZE' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Undo outfit' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Redo outfit' })).toBeDisabled()
  })

  it('locks the whole bar while a roll loads', () => {
    renderBar({ canUndo: true, canRedo: true, rolling: true })
    for (const name of ['RANDOMIZE', 'Undo outfit', 'Redo outfit']) expect(screen.getByRole('button', { name })).toBeDisabled()
  })

  it('stays on screen but locked on the Emotes tab', () => {
    renderBar({ canUndo: true }, { initialTab: 'emotes' })
    expect(screen.getByRole('button', { name: 'RANDOMIZE' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Undo outfit' })).toBeDisabled()
  })

  it('locks RANDOMIZE until the avatar has a body shape', () => {
    renderBar({}, { bodyShape: null })
    expect(screen.getByRole('button', { name: 'RANDOMIZE' })).toBeDisabled()
  })
})

describe('backpack randomize session', () => {
  it('sends each step to the bridge and follows the history it reports', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().backpack.toggle())
    act(() => h.session().backpack.randomizeOutfit())
    act(() => h.session().backpack.undoOutfit())
    act(() => h.session().backpack.redoOutfit())
    expect(h.driver.sentOf('randomizeOutfit')).toHaveLength(1)
    expect(h.driver.sentOf('undoOutfit')).toHaveLength(1)
    expect(h.driver.sentOf('redoOutfit')).toHaveLength(1)

    act(() => h.driver.emit({ kind: 'outfitHistory', canUndo: true, canRedo: false, rolling: false }))
    expect(h.session().backpack.outfitHistory).toEqual({ canUndo: true, canRedo: false, rolling: false })
  })

  it('closing the Backpack forgets the history', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().backpack.toggle())
    act(() => h.driver.emit({ kind: 'outfitHistory', canUndo: true, canRedo: true, rolling: false }))
    act(() => h.session().backpack.toggle())
    expect(h.driver.sentOf('resetOutfitHistory')).toHaveLength(1)
    expect(h.session().backpack.outfitHistory).toEqual({ canUndo: false, canRedo: false, rolling: false })
  })
})

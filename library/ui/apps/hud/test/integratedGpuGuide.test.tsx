// DOMAIN: the integrated-GPU guide (features/gate/IntegratedGpuGuide.tsx) — the dialog a Windows
// player on an Intel adapter gets once signed in, with the browser flag that moves it to the
// dedicated GPU, and the two ways out of it (lib/integratedGpu.ts decides when it opens).

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PopupHost, closeTopPopup, resetPopups } from '../design'
import { openIntegratedGpuGuide } from '../features/gate/IntegratedGpuGuide'
import { isInputLocked } from '@explorer-core/lib/inputLock'

// One host per test: a second `render(<PopupHost />)` would show every popup twice.
beforeEach(() => {
  render(<PopupHost />)
})
afterEach(resetPopups)

const adapter = { vendor: 'intel', architecture: 'gen-12lp', device: '', description: 'Intel(R) Iris(R) Xe Graphics' }

function open(onDontShowAgain = vi.fn()): ReturnType<typeof vi.fn> {
  act(() => {
    openIntegratedGpuGuide({ adapter, onDontShowAgain })
  })
  return onDontShowAgain
}

describe('openIntegratedGpuGuide', () => {
  it('names the adapter and gives the flag as a copyable target, never a link', () => {
    open()
    expect(screen.getByRole('dialog', { name: /dedicated graphics card/i })).toBeInTheDocument()
    expect(screen.getByText('Intel(R) Iris(R) Xe Graphics')).toBeInTheDocument()
    expect(screen.getByText('chrome://flags/#force-high-performance-gpu')).toBeInTheDocument()
    expect(screen.queryAllByRole('link')).toEqual([])
    expect(screen.getAllByRole('button', { name: /^Copy$/ })).toHaveLength(1)
  })

  it('"Don\'t show again" tells the owner once and closes; "Later" only closes', async () => {
    const remember = open()
    await userEvent.click(screen.getByRole('button', { name: /don.t show again/i }))
    expect(remember).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('dialog')).toBeNull()

    const later = open()
    await userEvent.click(screen.getByRole('button', { name: /^Later$/ }))
    expect(later).not.toHaveBeenCalled()
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('is an ordinary popup: the cancel action closes it without remembering, and it never locks input', () => {
    const remember = open()
    expect(isInputLocked()).toBe(false)
    act(() => closeTopPopup())
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(remember).not.toHaveBeenCalled()
  })
})

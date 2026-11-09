// DOMAIN: the integrated-GPU guide (features/gate/IntegratedGpuGuide.tsx) — the dialog a Windows
// player on an Intel adapter gets once the HUD is up, with both ways to move the browser to the
// dedicated GPU, and the two ways out of it (lib/integratedGpu.ts decides when it opens).

import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PopupHost, closeTopPopup, resetPopups } from '../design'
import { openIntegratedGpuGuide, WINDOWS_GRAPHICS_SETTINGS } from '../features/gate/IntegratedGpuGuide'
import { isInputLocked } from '../lib/inputLock'

afterEach(resetPopups)

const adapter = { vendor: 'intel', architecture: 'gen-12lp', device: '', description: 'Intel(R) Iris(R) Xe Graphics' }

function open(onDontShowAgain = vi.fn()): ReturnType<typeof vi.fn> {
  render(<PopupHost />)
  act(() => {
    openIntegratedGpuGuide({ adapter, onDontShowAgain })
  })
  return onDontShowAgain
}

describe('openIntegratedGpuGuide', () => {
  it('names the adapter and gives both fixes as copyable targets, never links', () => {
    open()
    expect(screen.getByRole('dialog', { name: /dedicated graphics card/i })).toBeInTheDocument()
    expect(screen.getByText('Intel(R) Iris(R) Xe Graphics')).toBeInTheDocument()
    expect(screen.getByText(WINDOWS_GRAPHICS_SETTINGS)).toBeInTheDocument()
    expect(screen.getByText('chrome://flags/#force-high-performance-gpu')).toBeInTheDocument()
    expect(screen.queryAllByRole('link')).toEqual([])
    expect(screen.getAllByRole('button', { name: /^Copy$/ })).toHaveLength(2)
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

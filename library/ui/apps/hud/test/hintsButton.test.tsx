import { describe, it, expect } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { KeyCap } from '@dcl/one-ui'
import { HintsButton } from '../design'
import { dispatchCancelLayer } from '@explorer-core/lib/cancelLayers'

// The HUD's HintsButton is the shared one (library/ui/components/HintsButton, tested there) with the HUD's
// keyboard icon, registered as a cancel layer while its list is open.
describe('HintsButton', () => {
  const hints = [{ icon: <KeyCap label="E" />, text: 'Press E to open emotes.' }]

  it('draws the HUD keyboard icon as a mask, and is no cancel layer while closed', () => {
    render(<HintsButton hints={hints} />)
    const button = screen.getByRole('button', { name: 'Controls' })
    expect(button.querySelector('svg')).toBeNull()
    expect(button.firstElementChild?.getAttribute('style')).toContain('mask')
    expect(dispatchCancelLayer()).toBe(false)
  })

  it('Cancel closes the open list, once', () => {
    render(<HintsButton hints={hints} label="Emote hints" className="mine" />)
    const button = screen.getByRole('button', { name: 'Emote hints' })
    expect(button.parentElement?.className).toContain('mine')
    fireEvent.click(button)
    expect(screen.getByRole('list', { name: 'Emote hints' }).textContent).toBe('EPress E to open emotes.')
    act(() => { expect(dispatchCancelLayer()).toBe(true) })
    expect(screen.queryByRole('list')).toBeNull()
    expect(dispatchCancelLayer()).toBe(false)
  })

  it('a click outside, a second press and an unmount each leave no cancel layer', () => {
    const view = render(<HintsButton hints={hints} />)
    const button = screen.getByRole('button', { name: 'Controls' })
    fireEvent.click(button)
    fireEvent.mouseDown(document.body)
    expect(screen.queryByRole('list')).toBeNull()
    expect(dispatchCancelLayer()).toBe(false)
    fireEvent.click(button)
    fireEvent.click(button)
    expect(dispatchCancelLayer()).toBe(false)
    fireEvent.click(button)
    view.unmount()
    expect(dispatchCancelLayer()).toBe(false)
  })
})

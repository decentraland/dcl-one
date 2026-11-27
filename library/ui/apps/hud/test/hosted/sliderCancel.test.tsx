// The cancel key on a focused slider (the colour bars, the Volume slider): the slider keeps its keys from the engine,
// which never hears that key, so the session closes the top layer itself on the first press, as the engine's Cancel
// does from a button. A text field still spends its first press leaving the field, and a switch leaves the key to the
// engine (bindingLabels states the rule; react-web's bootKeyForwarding test holds the web page to it).
import { describe, it, expect, afterEach, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { renderSession, enterAsGuest, type Harness } from '../harness'
import { ColorPicker, openPopup, resetPopups, PopupHost } from '../../design'
import { setBindingsSnapshot } from '../../lib/bindingLabels'
import { lockInput } from '@explorer-core/lib/inputLock'

async function world(): Promise<Harness> {
  const h = renderSession({ userId: null })
  await enterAsGuest(h)
  return h
}

const cancel = { kind: 'systemAction', action: 'Cancel', pressed: true } as const

/** The Backpack open, with the hair colour popup over it; its Hue bar dragged, so it holds the focus. */
async function draggedHueBar(): Promise<{ h: Harness; bar: HTMLInputElement }> {
  const h = await world()
  act(() => h.session().backpack.toggle())
  render(<ColorPicker label="Hair color" value="#5b310f" presets={['#1c1c1c', '#5b310f']} onChange={vi.fn()} />)
  fireEvent.click(screen.getByRole('button', { name: 'Hair color' }))
  const bar = screen.getByRole<HTMLInputElement>('slider', { name: 'Hue' })
  act(() => bar.focus())
  fireEvent.change(bar, { target: { value: '0.5' } })
  expect(document.activeElement).toBe(bar)
  return { h, bar }
}

const popup = (): HTMLElement | null => screen.queryByRole('dialog', { name: 'Hair color' })

/** A popup holding one control, which takes the focus. */
function popupWith(control: React.JSX.Element): HTMLElement {
  render(<PopupHost />)
  act(() => {
    openPopup(() => <div data-testid="popup">{control}</div>)
  })
  const focused = screen.getByTestId('popup').firstElementChild as HTMLElement
  act(() => focused.focus())
  return focused
}

afterEach(() => {
  resetPopups()
  act(() => setBindingsSnapshot([])) // module store leaks across tests
  document.body.innerHTML = ''
})

describe('the cancel key on a focused slider', () => {
  it('closes the colour popup on the first press, leaves the bar and keeps the Backpack open', async () => {
    const { h, bar } = await draggedHueBar()
    act(() => {
      fireEvent.keyDown(bar, { key: 'Escape', code: 'Escape' })
    })
    expect(popup()).toBeNull()
    expect(document.activeElement).not.toBe(bar)
    expect(h.session().backpack.open).toBe(true)
  })

  it('closes one layer per press: the next press, now the engine Cancel, closes the Backpack', async () => {
    const { h, bar } = await draggedHueBar()
    act(() => {
      fireEvent.keyDown(bar, { key: 'Escape', code: 'Escape' })
    })
    expect(h.session().backpack.open).toBe(true)
    act(() => h.driver.emit(cancel))
    expect(h.session().backpack.open).toBe(false)
  })

  it('leaves the keys that adjust the bar with it: nothing closes and the bar keeps the focus', async () => {
    const { h, bar } = await draggedHueBar()
    for (const code of ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown']) {
      act(() => {
        fireEvent.keyDown(bar, { key: code, code })
      })
    }
    expect(popup()).not.toBeNull()
    expect(document.activeElement).toBe(bar)
    expect(h.session().backpack.open).toBe(true)
  })

  it('follows a printable rebound cancel key, which a bar does not type; Escape then does nothing', async () => {
    const { h, bar } = await draggedHueBar()
    act(() => setBindingsSnapshot([[{ System: 'Cancel' }, ['KeyR']]]))
    act(() => {
      fireEvent.keyDown(bar, { key: 'Escape', code: 'Escape' })
    })
    expect(popup()).not.toBeNull()
    expect(document.activeElement).toBe(bar)
    act(() => {
      fireEvent.keyDown(bar, { key: 'r', code: 'KeyR' })
    })
    expect(popup()).toBeNull()
    expect(h.session().backpack.open).toBe(true)
  })

  it('does nothing while Cancel is unbound', async () => {
    const { bar } = await draggedHueBar()
    act(() => setBindingsSnapshot([[{ System: 'Cancel' }, []], [{ System: 'Map' }, ['KeyM']]]))
    act(() => {
      fireEvent.keyDown(bar, { key: 'Escape', code: 'Escape' })
    })
    expect(popup()).not.toBeNull()
    expect(document.activeElement).toBe(bar)
  })

  it('closes nothing while the input lock is held (crash modal), as the engine Cancel does', async () => {
    const { bar } = await draggedHueBar()
    let unlock!: () => void
    act(() => {
      unlock = lockInput()
    })
    act(() => {
      fireEvent.keyDown(bar, { key: 'Escape', code: 'Escape' })
    })
    expect(popup()).not.toBeNull()
    act(() => unlock())
  })

  it('closes a popup a slider sits in (the shared Slider, as in the Volume panel)', async () => {
    await world()
    const slider = popupWith(<input type="range" aria-label="Master" />)
    act(() => {
      fireEvent.keyDown(slider, { key: 'Escape', code: 'Escape' })
    })
    expect(screen.queryByTestId('popup')).toBeNull()
  })
})

describe('the cancel key elsewhere (unchanged)', () => {
  it('in a text field leaves the field first; the popup closes on the engine Cancel after it', async () => {
    const h = await world()
    const field = popupWith(<input type="text" />)
    act(() => {
      fireEvent.keyDown(field, { key: 'w', code: 'KeyW' })
    })
    expect(document.activeElement).toBe(field)
    act(() => {
      fireEvent.keyDown(field, { key: 'Escape', code: 'Escape' })
    })
    expect(document.activeElement).not.toBe(field)
    expect(screen.getByTestId('popup')).toBeTruthy()
    act(() => h.driver.emit(cancel))
    expect(screen.queryByTestId('popup')).toBeNull()
  })

  it('in a text field types a printable rebound cancel key: the field keeps the focus', async () => {
    await world()
    act(() => setBindingsSnapshot([[{ System: 'Cancel' }, ['KeyR']]]))
    const field = popupWith(<textarea />)
    act(() => {
      fireEvent.keyDown(field, { key: 'r', code: 'KeyR' })
    })
    expect(document.activeElement).toBe(field)
    expect(screen.getByTestId('popup')).toBeTruthy()
  })

  it.each([
    ['switch', <input key="switch" type="checkbox" role="switch" />],
    ['radio', <input key="radio" type="radio" name="level" />]
  ])('on a %s is left to the engine, whose Cancel closes the popup', async (_, element) => {
    const h = await world()
    const control = popupWith(element)
    act(() => {
      fireEvent.keyDown(control, { key: 'Escape', code: 'Escape' })
    })
    expect(screen.getByTestId('popup')).toBeTruthy()
    expect(document.activeElement).toBe(control)
    act(() => h.driver.emit(cancel))
    expect(screen.queryByTestId('popup')).toBeNull()
  })
})

// The cancel key on a focused slider (the colour bars, the Volume slider): the slider keeps its keys from the engine,
// which never hears that key, so the session closes the top layer itself on the first press, as the engine's Cancel
// does from a button. A text field still spends its first press leaving the field, and a switch leaves the key to the
// engine (bindingLabels states the rule; react-web's bootKeyForwarding test holds the web page to it).
import { describe, it, expect, afterEach, vi } from 'vitest'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { renderSession, enterAsGuest, type Harness } from '../harness'
import { ColorPicker, openPopup, resetPopups, PopupHost } from '../../design'
import { setBindingsSnapshot } from '../../lib/bindingLabels'
import { lockInput } from '@explorer-core/lib/inputLock'
import { Slider } from '@dcl/one-ui'
import { SLIDER_KEYS } from '../sliderKeys'

async function world(): Promise<Harness> {
  const h = renderSession({ userId: null })
  await enterAsGuest(h)
  return h
}

const cancel = { kind: 'systemAction', action: 'Cancel', pressed: true } as const

/** The engine's word on the keyboard (boot.js on the web, the CEF bridge natively): it reports every keyboard claim,
 *  the HUD's declared text focus included, and lets it go a few frames after the HUD does. */
const engineKeyboard = window as Window & { __engineTextFocus?: boolean }

/** The Backpack open, with the hair colour popup over it; its Hue bar dragged, so it holds the focus, and the engine
 *  reports the keyboard claimed by it. */
async function draggedHueBar(): Promise<{ h: Harness; bar: HTMLInputElement }> {
  const h = await world()
  act(() => h.session().backpack.toggle())
  render(<ColorPicker label="Hair color" value="#5b310f" presets={['#1c1c1c', '#5b310f']} onChange={vi.fn()} />)
  fireEvent.click(screen.getByRole('button', { name: 'Hair color' }))
  const bar = screen.getByRole<HTMLInputElement>('slider', { name: 'Hue' })
  act(() => bar.focus())
  fireEvent.change(bar, { target: { value: '0.5' } })
  expect(document.activeElement).toBe(bar)
  engineKeyboard.__engineTextFocus = true
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
  engineKeyboard.__engineTextFocus = false
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
    // until the engine lets the bar's claim go, a Cancel it resolved is still dropped (the declaration race)
    act(() => h.driver.emit(cancel))
    expect(h.session().backpack.open).toBe(true)
    engineKeyboard.__engineTextFocus = false
    act(() => h.driver.emit(cancel))
    expect(h.session().backpack.open).toBe(false)
  })

  it('leaves the keys that adjust the bar with it: nothing closes and the bar keeps the focus', async () => {
    const { h, bar } = await draggedHueBar()
    for (const code of SLIDER_KEYS) {
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

  it('closes a popup the shared Slider sits in, as the Volume panel holds it', async () => {
    await world()
    const slider = popupWith(<Slider variant="rail" value={50} min={0} max={100} step={1} onChange={vi.fn()} aria-label="Master" />)
    expect(slider).toBe(screen.getByRole('slider', { name: 'Master' }))
    engineKeyboard.__engineTextFocus = true
    act(() => {
      fireEvent.keyDown(slider, { key: 'Escape', code: 'Escape' })
    })
    expect(screen.queryByTestId('popup')).toBeNull()
  })

  it('declares text focus for the bar, the claim the engine then reports', async () => {
    const { h } = await draggedHueBar()
    await waitFor(() => expect(h.driver.last('uiFocus')).toMatchObject({ kind: 'uiFocus', text: true }))
  })

  it("natively leaves the press to the engine before the bar's claim lands: one close in all", async () => {
    // A native engine still hears the key until the HUD's text focus reaches it, and resolves Cancel itself. On the
    // web the page withholds the key either way, so a press in that window only leaves the bar.
    const { h, bar } = await draggedHueBar()
    engineKeyboard.__engineTextFocus = false
    act(() => {
      fireEvent.keyDown(bar, { key: 'Escape', code: 'Escape' })
    })
    expect(popup()).not.toBeNull()
    expect(document.activeElement).not.toBe(bar)
    act(() => h.driver.emit(cancel))
    expect(popup()).toBeNull()
    expect(h.session().backpack.open).toBe(true)
  })

  it('leaves a press a widget took (preventDefault) with that widget: nothing closes and the bar keeps the focus', async () => {
    const { h, bar } = await draggedHueBar()
    bar.addEventListener('keydown', (e) => e.preventDefault())
    act(() => {
      fireEvent.keyDown(bar, { key: 'Escape', code: 'Escape' })
    })
    expect(popup()).not.toBeNull()
    expect(document.activeElement).toBe(bar)
    expect(h.session().backpack.open).toBe(true)
  })

  it.each(['ctrlKey', 'altKey', 'metaKey'])('closes nothing on a chord with %s, as from a button', async (modifier) => {
    const { h, bar } = await draggedHueBar()
    act(() => {
      fireEvent.keyDown(bar, { key: 'Escape', code: 'Escape', [modifier]: true })
    })
    expect(popup()).not.toBeNull()
    expect(h.session().backpack.open).toBe(true)
  })

  it('closes nothing on Ctrl and a rebound printable cancel key', async () => {
    const { bar } = await draggedHueBar()
    act(() => setBindingsSnapshot([[{ System: 'Cancel' }, ['KeyR']]]))
    act(() => {
      fireEvent.keyDown(bar, { key: 'r', code: 'KeyR', ctrlKey: true })
    })
    expect(popup()).not.toBeNull()
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

import { describe, expect, it, vi } from 'vitest'
import { buttonSoundHandlers } from './buttonSounds'

// DOMAIN: which of the chat's buttons sound, on a click and on the pointer coming onto one.
describe('chat button sounds', () => {
  const setup = (html: string): { button: HTMLButtonElement; click: () => string[]; over: (from?: Element | null) => string[] } => {
    document.body.innerHTML = html
    const button = document.querySelector('button')!
    const play = vi.fn()
    const handlers = buttonSoundHandlers(play)
    const played = (): string[] => {
      const sounds = play.mock.calls.map((c) => c[0] as string)
      play.mockClear()
      return sounds
    }
    return {
      button,
      click: () => {
        handlers.onClickCapture({ target: button.firstElementChild ?? button } as unknown as React.MouseEvent)
        return played()
      },
      over: (from = null) => {
        handlers.onPointerOver({ target: button, relatedTarget: from } as unknown as React.PointerEvent)
        return played()
      }
    }
  }

  it('a button presses and highlights, from any of its children', () => {
    const b = setup('<button><span></span></button>')
    expect(b.click()).toEqual(['buttonPress'])
    expect(b.over()).toEqual(['buttonHover'])
    // moving between the button's own children is not coming onto it again
    expect(b.over(b.button.firstElementChild)).toEqual([])
  })

  it('data-ui-sound picks the softer press, or a press with no highlight', () => {
    const soft = setup('<button data-ui-sound="soft"></button>')
    expect(soft.click()).toEqual(['buttonClose'])
    expect(soft.over()).toEqual(['buttonHover'])
    const press = setup('<button data-ui-sound="press"></button>')
    expect(press.click()).toEqual(['buttonPress'])
    expect(press.over()).toEqual([])
  })

  it('a disabled button is silent, aria-disabled included', () => {
    for (const html of ['<button disabled></button>', '<button aria-disabled="true"></button>']) {
      const b = setup(html)
      expect(b.click()).toEqual([])
      expect(b.over()).toEqual([])
    }
  })
})

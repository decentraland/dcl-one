import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, it, expect, vi } from 'vitest'
import { render } from '@testing-library/react'
import { BackpackPage } from '../../features/backpack/BackpackPage'
import styles from '../../features/backpack/BackpackPage.module.css'
import { fakeProfileState, fakeSession } from '../harness'

// The Backpack's backdrop leaves the avatar preview to the engine, but only once the engine has drawn
// the avatar there: before then what shows through is whatever the engine drew under it (the lobby's
// stage, the world).
describe('Backpack preview hole', () => {
  const page = (previewReady?: boolean, modal = false): HTMLElement => {
    const s = fakeSession()
    const { container } = render(
      <BackpackPage backpack={{ ...s.backpack, open: true }} emotes={s.emotes} profile={fakeProfileState()} onNavigate={vi.fn()} setEngineViewport={vi.fn()} modal={modal} {...(previewReady != null ? { previewReady } : {})} />
    )
    return container.querySelector(`.${styles.backdrop}`)!
  }

  it('keeps the backdrop whole until the preview is ready', () => {
    expect(page(false)).toHaveClass(styles.backdropShut)
  })

  it('opens the backdrop onto the preview once it is ready', () => {
    expect(page(true)).not.toHaveClass(styles.backdropShut)
  })

  // The modal over the lobby cuts its own, wider hole (`.inModal .backdrop`): the shut backdrop must
  // win over it too, or the lobby under the modal shows through. jsdom computes no `mask`, so this reads
  // the rule itself.
  it('keeps the backdrop whole over the lobby too', () => {
    const css = readFileSync(join(__dirname, '../../features/backpack/BackpackPage.module.css'), 'utf8')
    const shut = [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(([, selector]) => selector.includes('backdropShut'))
    expect(shut).toBeDefined()
    const selectors = shut![1].split(',').map((sel) => sel.trim())
    expect(selectors).toContain('.inModal .backdrop.backdropShut')
    expect(shut![2]).toMatch(/\bmask:\s*none\b/)
  })

  it('is open when the host says nothing of readiness', () => {
    expect(page()).not.toHaveClass(styles.backdropShut)
  })
})

import { describe, it, expect, afterEach, vi } from 'vitest'
import { getPreferredLocale, useLocale } from './locale'

describe('locale store', () => {
  afterEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  it('defaults to en when nothing is saved', () => {
    expect(getPreferredLocale()).toBe('en')
  })

  it('persists the chosen locale and exposes it as state', () => {
    useLocale.getState().setLocale('es')
    expect(useLocale.getState().locale).toBe('es')
    expect(localStorage.getItem('creators:locale')).toBe('es')
    expect(getPreferredLocale()).toBe('es')
  })

  it.each([
    ['zh-CN', 'zh'],
    ['zh-TW', 'zh'],
    ['zh_CN', 'zh'],
    ['es-AR', 'es'],
    ['fr-FR', 'en']
  ])('maps the browser language %s to %s when nothing is saved', (language, expected) => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue(language)
    expect(getPreferredLocale()).toBe(expected)
  })

  it('ignores an invalid saved value', () => {
    localStorage.setItem('creators:locale', 'fr')
    expect(getPreferredLocale()).toBe('en')
  })
})

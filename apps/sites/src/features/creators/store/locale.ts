import { create } from 'zustand'

const KEY = 'creators:locale'

export const LOCALES = ['en', 'es', 'zh'] as const

export type Locale = (typeof LOCALES)[number]

// Initial locale: saved choice → browser language → English.
export const getPreferredLocale = (): Locale => {
  try {
    const saved = localStorage.getItem(KEY)
    if (saved && (LOCALES as readonly string[]).includes(saved)) return saved as Locale
  } catch {
    // restricted storage → fall through to browser language
  }
  // Primary subtag only (`-` or `_`): zh-TW/zh-HK get Simplified, the closest we have, and `zha` doesn't match.
  const primary = navigator.language?.toLowerCase().split(/[-_]/)[0] ?? ''
  return LOCALES.find(locale => locale === primary) ?? 'en'
}

type LocaleState = {
  locale: Locale
  setLocale: (locale: Locale) => void
}

export const useLocale = create<LocaleState>(set => ({
  locale: getPreferredLocale(),
  setLocale: locale => {
    try {
      localStorage.setItem(KEY, locale)
    } catch {
      // ignore — the choice just won't survive a reload
    }
    set({ locale })
  }
}))

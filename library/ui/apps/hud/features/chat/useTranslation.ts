// The chat's translation state as React state (library/explorer-core/features/chat/translation.ts).
import { useSyncExternalStore } from 'react'
import { subscribeTranslationPrefs, subscribeTranslations, translationPrefs, translationsVersion, type TranslationPrefs } from '@explorer-core/features/chat/translation'

export function useTranslationPrefs(): TranslationPrefs {
  return useSyncExternalStore(subscribeTranslationPrefs, translationPrefs)
}

/** Re-renders when any translation changes; read them with getTranslation. */
export function useTranslations(): number {
  return useSyncExternalStore(subscribeTranslations, translationsVersion)
}

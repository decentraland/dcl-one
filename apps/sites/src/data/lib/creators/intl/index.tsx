import { ReactNode, useCallback, useLayoutEffect } from 'react'
import { IntlProvider, useIntl } from 'react-intl'
import { useLocale, type Locale } from '~/store/locale'
import { flattenMessages } from '~/lib/messages'
import en from './en.json'
import es from './es.json'
import zh from './zh.json'

const messages: Record<Locale, Record<string, string>> = {
  en: flattenMessages(en),
  es: flattenMessages(es),
  zh: flattenMessages(zh)
}

const TranslationProvider = ({ children }: { children: ReactNode }) => {
  const locale = useLocale(s => s.locale)
  // Without zh-Hans, browsers may pick Japanese glyph variants for shared characters and screen readers an English voice.
  useLayoutEffect(() => {
    document.documentElement.lang = locale === 'zh' ? 'zh-Hans' : locale
  }, [locale])
  return (
    <IntlProvider locale={locale} defaultLocale="en" messages={messages[locale]} fallbackOnEmptyString={false}>
      {children}
    </IntlProvider>
  )
}

type PlainValues = Record<string, string | number | ((chunks: string[]) => string)>
type RichValues = Record<string, string | number | ((chunks: ReactNode[]) => ReactNode)>
type Translate = (id: string, values?: PlainValues) => string
type TranslateRich = (id: string, values: RichValues) => ReactNode

const useTranslation = (): { t: Translate; rich: TranslateRich } => {
  const intl = useIntl()
  // Stable across renders so `t` can sit in hook dependency arrays without defeating them.
  const t = useCallback((id: string, values?: PlainValues) => intl.formatMessage({ id }, values), [intl])
  const rich = useCallback((id: string, values: RichValues) => intl.formatMessage({ id }, values), [intl])
  return { t, rich }
}

/** The English copy of a message, for analytics values that must not change with the visitor's language. */
const englishMessage = (id: string): string => messages.en[id] ?? id

export { TranslationProvider, englishMessage, useTranslation, type Translate, type TranslateRich }

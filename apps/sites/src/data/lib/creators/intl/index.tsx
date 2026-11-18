import { Fragment, type ReactNode } from 'react'
import { flattenMessages } from '@/data/lib/creators/messages'
import en from './en.json'
import { formatParts, type FormatValue } from './format'

const LOCALE = 'en'
const messages: Record<string, string> = flattenMessages(en)

const TranslationProvider = ({ children }: { children: ReactNode }) => <>{children}</>

type PlainValues = Record<string, string | number | ((chunks: string[]) => string)>
type RichValues = Record<string, ReactNode | ((chunks: ReactNode[]) => ReactNode)>
type Translate = (id: string, values?: PlainValues) => string
type TranslateRich = (id: string, values: RichValues) => ReactNode

const t: Translate = (id, values) => {
  const message = messages[id]
  return message === undefined ? id : formatParts(message, values as Record<string, FormatValue>).join('')
}

const rich: TranslateRich = (id, values) => {
  const message = messages[id]
  if (message === undefined) return id
  const parts = formatParts(message, values as Record<string, FormatValue>) as ReactNode[]
  return parts.map((part, index) => <Fragment key={index}>{part}</Fragment>)
}

const translation = { t, rich }

/** `t` and `rich` are the same functions on every render, so they can sit in hook dependency arrays. */
const useTranslation = (): { t: Translate; rich: TranslateRich } => translation

const intl = {
  locale: LOCALE,
  formatMessage: ({ id }: { id: string }, values?: RichValues): ReactNode => (values ? rich(id, values) : t(id)),
  formatNumber: (value: number | bigint, options?: Intl.NumberFormatOptions) =>
    new Intl.NumberFormat(LOCALE, options).format(value),
  formatDate: (value: number | string | Date, options?: Intl.DateTimeFormatOptions) =>
    new Intl.DateTimeFormat(LOCALE, options).format(new Date(value))
}

const useIntl = () => intl

/** The copy of a message, for analytics values. */
const englishMessage = (id: string): string => messages[id] ?? id

export { TranslationProvider, englishMessage, useIntl, useTranslation, type Translate, type TranslateRich }

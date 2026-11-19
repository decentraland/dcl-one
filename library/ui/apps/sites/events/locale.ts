import { createContext, useContext } from 'react'
import { messages } from './messages'

const translationIndexes = { es: 0, fr: 1, zh: 2, ko: 3, ja: 4 } as const

export type EventsLocale = 'en' | keyof typeof translationIndexes
type Message = keyof typeof messages
type Values = Record<string, string | number>

export const EventsLocaleContext = createContext<EventsLocale>('en')

export function eventText(locale: EventsLocale, message: Message, values: Values = {}): string {
  const translated = locale === 'en' ? message : messages[message][translationIndexes[locale]]
  return translated.replace(/\{(\w+)\}/g, (placeholder, key: string) => {
    const value = values[key]
    return typeof value === 'number'
      ? new Intl.NumberFormat(locale).format(value)
      : (value ?? placeholder)
  })
}

export function useEventsCopy() {
  const locale = useContext(EventsLocaleContext)
  return {
    locale,
    text: (message: Message, values?: Values) => eventText(locale, message, values),
    errorText: (message: string) =>
      Object.hasOwn(messages, message) ? eventText(locale, message as Message) : message
  }
}

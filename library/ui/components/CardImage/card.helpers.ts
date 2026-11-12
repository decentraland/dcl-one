export function hasUnsafeURLCharacters(value: string): boolean {
  return (
    value.includes('\\') ||
    [...value].some((character) => {
      const code = character.charCodeAt(0)
      return code < 32 || code === 127
    })
  )
}
export function cardHref(value: string | undefined, allowLaunch = false): string | undefined {
  const href = value?.trim()
  if (!href || hasUnsafeURLCharacters(href)) return undefined
  try {
    const url = new URL(href, 'https://card.invalid')
    if (url.username || url.password) return undefined
    return ['https:', 'http:'].includes(url.protocol) ||
      (allowLaunch && ['decentraland:', 'intent:'].includes(url.protocol))
      ? href
      : undefined
  } catch {
    return undefined
  }
}
export function knownCount(value: number | null | undefined): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

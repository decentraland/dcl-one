import type { ComponentPropsWithoutRef, ElementType } from 'react'

/**
 * The props of a part that renders an anchor, or the caller's link component passed as `as`: the part's
 * own props, then every prop of that component but its children.
 */
export type PolymorphicLinkProps<C extends ElementType, Own> = Own & {
  as?: C
  className?: string
} & Omit<ComponentPropsWithoutRef<C>, 'as' | 'children' | 'className'>

export function hasUnsafeURLCharacters(value: string): boolean {
  return (
    value.includes('\\') ||
    [...value].some((character) => {
      const code = character.charCodeAt(0)
      return code < 32 || code === 127
    })
  )
}

export function secureRel(target: string | undefined, rel: string | undefined): string | undefined {
  if (!target || ['_self', '_parent', '_top'].includes(target.toLowerCase())) return rel
  return [...new Set([...(rel ?? '').split(/\s+/).filter(Boolean), 'noopener', 'noreferrer'])].join(
    ' '
  )
}

export function isPlainLinkClick(
  event: Pick<
    MouseEvent,
    'defaultPrevented' | 'button' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'
  >
): boolean {
  return (
    !event.defaultPrevented &&
    event.button === 0 &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.altKey &&
    !event.shiftKey
  )
}

/** Launch protocols are only for trusted, application-built destinations. */
export function linkHref(value: unknown, allowLaunch = false): string | undefined {
  if (typeof value !== 'string') return undefined
  const href = value.trim()
  if (!href || hasUnsafeURLCharacters(href)) return undefined
  try {
    const url = new URL(href, 'https://link.invalid')
    if (url.username || url.password) return undefined
    return ['https:', 'http:'].includes(url.protocol) ||
      (allowLaunch && ['decentraland:', 'intent:'].includes(url.protocol))
      ? href
      : undefined
  } catch {
    return undefined
  }
}

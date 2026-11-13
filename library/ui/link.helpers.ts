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

import { Fragment, isValidElement, type ReactNode } from 'react'
export function hasRenderableContent(content: ReactNode): boolean {
  if (content == null || typeof content === 'boolean' || content === '') return false
  if (Array.isArray(content)) return content.some(hasRenderableContent)
  return true
}

function hasMeaningfulContent(content: ReactNode): boolean {
  if (!hasRenderableContent(content)) return false
  if (typeof content === 'string') return Boolean(content.trim())
  if (Array.isArray(content)) return content.some(hasMeaningfulContent)
  if (isValidElement<{ children?: ReactNode }>(content) && content.type === Fragment)
    return hasMeaningfulContent(content.props.children)
  return true
}
/** Accessible labels use authored strings; numeric content belongs to contentFallback. */
export function contentLabel(value: unknown, fallback: string): string {
  return typeof value === 'string' && value.trim() ? value.trim() : fallback
}
export function contentFallback(content: ReactNode, fallback: ReactNode): ReactNode {
  return hasMeaningfulContent(content) ? content : fallback
}

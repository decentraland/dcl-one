import type { ReactNode } from 'react'
export function hasRenderableContent(content: ReactNode): boolean {
  if (content == null || typeof content === 'boolean' || content === '') return false
  if (Array.isArray(content)) return content.some(hasRenderableContent)
  return true
}

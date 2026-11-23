import { createContext, useContext } from 'react'

export const ThumbnailPublicBase = createContext<string | undefined>(undefined)

export function useThumbnailPublicBase(): string | undefined {
  const base = useContext(ThumbnailPublicBase)
  return base ?? (typeof window !== 'undefined' ? window.location.origin : undefined)
}

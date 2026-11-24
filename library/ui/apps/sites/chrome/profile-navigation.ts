import { createContext, type ReactNode } from 'react'

type ProfileTab = { id: string; label: ReactNode; href?: string }
export type ProfileNavigationProps = {
  tabs: readonly ProfileTab[]
  active?: string
  onTab?: (id: string) => void
  label?: string
}

// The route supplies real destinations once for the shared secondary navigation.
export const ProfileNavigationContext = createContext<ProfileNavigationProps | null>(null)

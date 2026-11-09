import { createContext, type ReactElement, type ReactNode, type InputHTMLAttributes } from 'react'

type NodeAction = {
  label: string
  onActivate: () => void
  pressed?: boolean
}

type FigmaInteractionBindings = {
  field?: {
    container: string
    control: ReactElement<InputHTMLAttributes<HTMLInputElement>>
  }
  tabList?: string
  replacement?: (name: string, path: string, text: string) => ReactNode | undefined
  action?: (name: string, path: string, text: string) => NodeAction | undefined
  disclosure?: {
    label: string
    open: boolean
    headerCount: number
    onToggle: () => void
    onClose: () => void
  }
}

/** Native controls are opt-in: a static gallery preview never provides these bindings. */
const FigmaInteractions = createContext<FigmaInteractionBindings | null>(null)

export { FigmaInteractions }
export type { FigmaInteractionBindings, NodeAction }

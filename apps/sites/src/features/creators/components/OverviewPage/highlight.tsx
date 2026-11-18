import { type ReactNode } from 'react'

/** Renders the `<hl>` tag of an overview heading as the gradient word. */
export const highlight = { hl: (chunks: ReactNode[]) => <span>{chunks}</span> }

/** Drops the `<hl>` tag, for accessible names built from the same heading. */
export const plainText = { hl: (chunks: string[]) => chunks.join('') }

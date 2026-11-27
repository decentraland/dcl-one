import { createContext } from 'react'

/**
 * Whether the canvas is one the Showcase draws inside itself (`embedded=1`). An example that opens on load takes the
 * keyboard focus, which is right on its own page and wrong partway down another: it starts closed there.
 */
export const EmbeddedCanvas = createContext(false)

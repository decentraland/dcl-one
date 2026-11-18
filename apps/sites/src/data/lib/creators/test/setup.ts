import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { TextDecoder, TextEncoder } from 'node:util'
import { afterEach } from 'vitest'

// jsdom's sandbox lacks the encoding classes the signing code needs.
globalThis.TextEncoder ??= TextEncoder as typeof globalThis.TextEncoder
globalThis.TextDecoder ??= TextDecoder as typeof globalThis.TextDecoder

// The dev build of a component expects the React refresh runtime of a dev page, which no test page has.
if (typeof window !== 'undefined') {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => undefined,
    $RefreshSig$: () => (type: unknown) => type
  })
}

afterEach(cleanup)

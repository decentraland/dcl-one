import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { TextDecoder, TextEncoder } from 'node:util'
import { afterEach } from 'vitest'

// jsdom's sandbox lacks the encoding classes the signing code needs.
globalThis.TextEncoder ??= TextEncoder as typeof globalThis.TextEncoder
globalThis.TextDecoder ??= TextDecoder as typeof globalThis.TextDecoder

afterEach(cleanup)

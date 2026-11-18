import '@testing-library/jest-dom/vitest'
import { TextDecoder, TextEncoder } from 'node:util'

// jsdom's sandbox lacks the encoding classes @dcl/crypto needs.
globalThis.TextEncoder ??= TextEncoder as typeof globalThis.TextEncoder
globalThis.TextDecoder ??= TextDecoder as typeof globalThis.TextDecoder

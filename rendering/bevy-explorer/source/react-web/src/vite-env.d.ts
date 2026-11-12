/// <reference types="vite/client" />

// Build-time constant (vite.config.ts `define`): true in the native HUD bundle
// (`bundle:native`, --mode native) and on the dev server; FALSE in deployed web builds,
// where the minifier deletes every ?native=1 branch — a link must not be able to put a
// web deployment into native mode (it would skip the launch gates).
declare const __NATIVE_HUD__: boolean

// Build-time constant (vite.config.ts `define`): true when the bundle was built with
// HUD_REACT_PROFILING=1 on React's profiling build; false otherwise and in the native bundle.
declare const __HUD_REACT_PROFILING__: boolean

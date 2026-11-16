/// <reference types="vite/client" />

// Build-time constant (vite.config.ts `define`): true in the native HUD bundle
// (`bundle:native`, --mode native) and on the dev server; FALSE in deployed web builds,
// where the minifier deletes every ?native=1 branch — a link must not be able to put a
// web deployment into native mode (it would skip the launch gates).
declare const __NATIVE_HUD__: boolean

// Build-time constant (vite.config.ts `define`): the short commit the HUD was built from, '' when
// the build had no git checkout. The sign-in screens show it in their corner.
declare const __HUD_BUILD__: string

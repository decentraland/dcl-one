// Every generated stylesheet sits in one of three cascade layers, so the cascade, not the order a bundle happens to
// load the files in, decides between them: the runtime reset loses to the foundations (tokens and text styles), and
// both lose to component styles. A consumer's unlayered CSS beats all three.
//
// Layer order is fixed by the first stylesheet that names a layer, and a bundle may load any of these files first,
// so each one repeats the full order statement; repeating it is a no-op.
const LAYER_ORDER = '@layer dcl.reset, dcl.foundations, dcl.components;'

type CssLayer = 'dcl.reset' | 'dcl.foundations' | 'dcl.components'

function inLayer(layer: CssLayer, css: string): string {
  return `${LAYER_ORDER}\n\n@layer ${layer} {\n${css.trimEnd()}\n}\n`
}

export { inLayer, LAYER_ORDER }
export type { CssLayer }

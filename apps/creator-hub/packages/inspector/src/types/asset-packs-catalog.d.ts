// The bundled catalog is a generated JSON file of several megabytes. Without this declaration the
// compiler infers a literal type for the whole file on every type check and declaration build.
declare module '@dcl/asset-packs/catalog.json' {
  import type { Catalog } from '@dcl/asset-packs';
  export const assetPacks: Catalog['assetPacks'];
  const catalog: Catalog;
  export default catalog;
}

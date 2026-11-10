type Platform = 'all' | 'web' | 'explorer' | 'bevy' | 'unity' | 'unclassified'
type NavigationVariant = { properties: Record<string, string> }
const platforms: Platform[] = ['all', 'web', 'explorer', 'bevy', 'unity', 'unclassified']
function variantPlatform(variant: NavigationVariant): Platform {
  const entry = Object.entries(variant.properties).find(([name]) =>
    ['product', 'platform'].includes(name.toLowerCase())
  )
  const value = entry?.[1].toLowerCase()
  return platforms.includes(value as Platform) && value !== 'all'
    ? (value as Platform)
    : 'unclassified'
}
function matchesPlatform(variant: NavigationVariant, platform: Platform): boolean {
  return platform === 'all' || variantPlatform(variant) === platform
}
export { platforms, variantPlatform, matchesPlatform }
export type { Platform }

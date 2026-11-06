import path from 'node:path'
import { listFiles, readIfExists, toPosix } from './fs-safe.js'

// Keep every source consumer, including foreign code whose ownership cannot authorize a deletion.
async function assetUsers(root: string, assetDirectory: string, sourceDirectory: string) {
  const assets = await listFiles(path.join(root, assetDirectory))
  const users = new Map(assets.map((file) => [toPosix(path.relative(root, file)), [] as string[]]))
  if (!assets.length) return users
  for (const file of await listFiles(path.join(root, sourceDirectory))) {
    if (!/\.(?:tsx?|css)$/.test(file)) continue
    const content = (await readIfExists(file)) ?? ''
    const relative = toPosix(path.relative(root, file))
    for (const [asset, consumers] of users)
      if (content.includes(path.basename(asset))) consumers.push(relative)
  }
  return users
}
export { assetUsers }

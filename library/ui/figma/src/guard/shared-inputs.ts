import path from 'node:path'
import { listFiles, readIfExists, toPosix } from './fs-safe.js'
import { ownerOf, type Ownership } from './ownership.js'

const GENERATED_ROOTS = ['figma/review/src/generated', 'components']
const variables = (text: string) => {
  const result = new Map<string, string[]>()
  for (const [, name, value] of text.matchAll(/(--[\w-]+)\s*:\s*([^;}]+)/g))
    result.set(name!, [...(result.get(name!) ?? []), value!.trim()])
  return result
}
const references = (text: string) => [...text.matchAll(/var\(\s*(--[\w-]+)/g)].map((m) => m[1]!)

// An unchanged component still changes appearance when a shared variable it uses changes.
async function sharedInputConflicts(
  baseDir: string,
  stagedDir: string,
  ownership: Ownership,
  captured: ReadonlySet<string>,
  directories: readonly string[]
): Promise<{ path: string; pageId: string; cause: string }[]> {
  const conflicts: { path: string; pageId: string; cause: string }[] = []
  if (path.resolve(baseDir) === path.resolve(stagedDir)) return conflicts
  for (const GENERATED of GENERATED_ROOTS) {
    if (
      !directories.some(
        (directory) =>
          directory === GENERATED ||
          GENERATED.startsWith(directory + '/') ||
          directory.startsWith(GENERATED + '/')
      )
    )
      continue
    const tokens = async (root: string) => {
      const files = await listFiles(path.join(root, GENERATED, 'tokens'))
      return variables(
        (
          await Promise.all(
            files.filter((file) => file.endsWith('.css')).map((file) => readIfExists(file))
          )
        ).join('\n')
      )
    }
    const before = await tokens(baseDir)
    const after = await tokens(stagedDir)
    const changed = new Set(
      [...before.keys(), ...after.keys()].filter(
        (name) => JSON.stringify(before.get(name)) !== JSON.stringify(after.get(name))
      )
    )
    let expanded = true
    while (expanded) {
      expanded = false
      for (const [name, values] of [...before, ...after])
        if (
          !changed.has(name) &&
          references(values.join('\n')).some((dependency) => changed.has(dependency))
        ) {
          changed.add(name)
          expanded = true
        }
    }
    const changedShared: string[] = []
    for (const directory of [
      'icons'
      // Upstream also scans the hand-written components next to the library's generated folder; here
      // they share components/ with the promoted ones, which this root already covers (layout.ts).
    ]) {
      const files = [
        ...new Set(
          [
            ...(await listFiles(path.join(baseDir, GENERATED, directory))),
            ...(await listFiles(path.join(stagedDir, GENERATED, directory)))
          ].map((file) =>
            file.startsWith(baseDir + path.sep)
              ? toPosix(path.relative(baseDir, file))
              : toPosix(path.relative(stagedDir, file))
          )
        )
      ]
      for (const file of files) {
        if (file.endsWith('.d.ts')) continue
        if (
          (await readIfExists(path.join(baseDir, file))) !==
          (await readIfExists(path.join(stagedDir, file)))
        )
          changedShared.push(file)
      }
    }
    if (!changed.size && !changedShared.length) continue
    for (const file of await listFiles(path.join(baseDir, GENERATED, 'components'))) {
      if (!/\.(?:css|tsx?)$/.test(file)) continue
      const relative = toPosix(path.relative(baseDir, file))
      const owner = ownerOf(ownership, relative)
      if (!owner || !('pageId' in owner) || captured.has(owner.pageId)) continue
      const text = (await readIfExists(file)) ?? ''
      const used = references(text).filter((name) => changed.has(name))
      const shared = changedShared.filter((input) =>
        input.includes('/icons/')
          ? /(?:\.\.\/)+icons(?:\/|['"])/.test(text) ||
            /(?:\.\.\/)+components\/figma-module-runtime/.test(text)
          : /(?:\.\.\/)+components\//.test(text)
      )
      if (used.length || shared.length)
        conflicts.push({
          path: relative,
          pageId: owner.pageId,
          cause: `shared input change affects an unselected page: ${[...new Set([...used, ...shared])].join(', ')}`
        })
    }
  }
  return conflicts
}
export { sharedInputConflicts }

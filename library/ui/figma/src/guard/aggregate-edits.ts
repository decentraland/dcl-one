import { aggregateKind, parseAggregate } from './aggregates.js'
import { atomicWrite, readIfExists, resolveContained } from './fs-safe.js'
import { libraryEntryPresent, LIBRARY_INDEX, LIBRARY_PROMOTION } from './library.js'
import type { Ownership } from './ownership.js'

async function artifactPresent(
  root: string,
  artifact: string,
  ownership: Ownership
): Promise<boolean> {
  const [file, key] = artifact.split('#') as [string, string | undefined]
  const text = await readIfExists(await resolveContained(root, file))
  if (text === undefined) return false
  if (key === undefined) return true
  if (file === LIBRARY_INDEX || file === LIBRARY_PROMOTION)
    return libraryEntryPresent(root, artifact)
  const kind = aggregateKind(file)
  return kind ? (parseAggregate(kind, text, ownership)?.entries.has(key) ?? false) : false
}

async function removeAggregateEntries(root: string, artifacts: string[]): Promise<string[]> {
  const byFile = new Map<string, Set<string>>()
  for (const artifact of artifacts) {
    const [file, key] = artifact.split('#') as [string, string | undefined]
    if (key !== undefined && file !== LIBRARY_INDEX && file !== LIBRARY_PROMOTION)
      byFile.set(file, (byFile.get(file) ?? new Set()).add(key))
  }
  const changed: string[] = []
  for (const [file, keys] of byFile) {
    const target = await resolveContained(root, file)
    const text = await readIfExists(target)
    if (text === undefined) continue
    const kind = aggregateKind(file)
    let next: string
    if (kind === 'components-index')
      next = text
        .split('\n')
        .filter((line) => {
          const match = /^export \* from '\.\/(.+)'$/.exec(line)
          return !match || !keys.has(`export:${match[1]}`)
        })
        .join('\n')
    else if (kind === 'components-manifest') {
      const start = text.indexOf('= [')
      if (start < 0) throw new Error(`Invalid component manifest: ${file}`)
      const entries = JSON.parse(text.slice(start + 2)) as { name: string }[]
      next =
        text.slice(0, start + 2) +
        JSON.stringify(
          entries.filter((entry) => !keys.has(`component:${entry.name}`)),
          null,
          2
        ) +
        '\n'
    } else {
      const data = JSON.parse(text)
      if (kind === 'snapshot')
        data.components = (data.components ?? []).filter(
          (component: { id: string }) => !keys.has(`component:${component.id}`)
        )
      else if (kind === 'catalog' || kind === 'promotion-manifest') {
        data.components = (data.components ?? []).filter(
          (component: { name: string }) => !keys.has(`component:${component.name}`)
        )
        if (data.excluded)
          data.excluded = data.excluded.filter(
            (component: { name: string }) => !keys.has(`excluded:${component.name}`)
          )
        if (data.files)
          for (const key of keys)
            if (key.startsWith('fingerprint:')) delete data.files[key.slice('fingerprint:'.length)]
      } else if (kind === 'inventory')
        for (const scope of ['review', 'promotion'])
          data[scope] = (data[scope] ?? [])
            .filter((page: { pageId: string }) => !keys.has(`${scope}:${page.pageId}`))
            .map((page: { pageId: string; componentIds?: string[] }) => ({
              ...page,
              ...(page.componentIds
                ? {
                    componentIds: page.componentIds.filter(
                      (id) => !keys.has(`${scope}:${page.pageId}:component:${id}`)
                    )
                  }
                : {})
            }))
      else throw new Error(`Unsupported retirement aggregate: ${file}`)
      next = JSON.stringify(data, null, 2) + '\n'
    }
    if (next !== text) {
      await atomicWrite(target, next)
      changed.push(...[...keys].map((key) => `${file}#${key}`))
    }
  }
  return changed
}
export { artifactPresent, removeAggregateEntries }

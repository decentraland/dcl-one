import { readdirSync, readFileSync } from 'fs'
import { pathExistsSync } from 'fs-extra'
import { resolve } from 'path'
import { flow, commonChecks } from './common'

import { itExecutes, patchJson, readJson } from './helpers'

flow('build-all', () => {
  commonChecks()

  const graph = getMonorepoGraph()

  function resolveProjectPath(packageName: string) {
    return resolve(process.cwd(), './packages/', packageName)
  }

  flow('monorepo has no dependency cycles', () => {
    detectCyclesTest(graph)
  })

  flow('update versions of packages.json copying it from @dcl/sdk', () => {
    const { version } = JSON.parse(readFileSync(resolve('./packages/@dcl/sdk/package.json'), 'utf8'))

    for (const dependency of processWithOptimisticDependencies(graph)) {
      const projectDirectory = resolveProjectPath(dependency)
      it(`set version ${version} in ${dependency}`, () => {
        patchJson('package.json', projectDirectory, ($) => ({ ...$, version }))
      })
    }
  })

  flow('pin linked dependencies and pack', () => {
    for (const dependency of processWithOptimisticDependencies(graph)) {
      const projectDirectory = resolveProjectPath(dependency)
      pinLinkedDependencies(projectDirectory)
      pinProtocol(projectDirectory)
      itExecutes('pnpm pack', projectDirectory)
    }
  })
})

// process dependencies
function* processWithOptimisticDependencies(graph: Map<string, Set<string>>): Iterable<string> {
  const cloneGraph = structuredClone(graph)
  let modified = false

  do {
    modified = false

    for (const [key, values] of cloneGraph) {
      if (!values.size) {
        for (const [, values] of cloneGraph) {
          values.delete(key)
        }

        cloneGraph.delete(key)
        yield key
        modified = true
        break
      }
    }
  } while (modified)

  if (cloneGraph.size) throw new Error('Could not process the graph optimistically')
}

// returns a map of <Lib, Dep[]>
function getMonorepoGraph(): Map<string, Set<string>> {
  const base = resolve('packages/@dcl')
  const packages = readdirSync(base)

  const graph = new Map<string, Set<string>>()

  for (const path of packages) {
    const packageJson = resolve(base, path, 'package.json')

    const deps = new Set<string>()

    const { dependencies, devDependencies } = JSON.parse(readFileSync(packageJson, 'utf-8'))

    for (const [key] of Object.entries({ ...dependencies, ...devDependencies } as Record<string, string>)) {
      if (key.startsWith('@dcl')) {
        const dependencyRootPath = resolve('packages', key)
        if (pathExistsSync(dependencyRootPath)) {
          deps.add(key)
        }
      }
    }

    graph.set(`@dcl/${path}`, deps)
  }
  return graph
}

function detectCyclesTest(graph: Map<string, Set<string>>) {
  const errors = new Set<string>()
  function getCycle(n: string, path: string[]) {
    if (path.includes(n)) {
      errors.add(`Cyclic dependencies ${path.slice(path.indexOf(n)).concat(n).join(' <- ')}`)
      return
    }
    path.push(n)
    graph.get(n)!.forEach((next) => getCycle(next, path.slice(0)))
  }
  it('check for cycles in imports', () => {
    Object.keys(graph).forEach((n) => getCycle(n, []))
    if (errors.size) throw new Error(Array.from(errors).join('\n'))
  })
}

// A workspace:* dependency is a sibling under packages/ and a link: one points at its folder; the packed
// manifest names the version each of them carries. @dcl/protocol is pinned by pinProtocol instead.
function pinLinkedDependencies(path: string) {
  const { dependencies, devDependencies } = JSON.parse(readFileSync(resolve(path, 'package.json'), 'utf-8'))

  for (const field of ['dependencies', 'devDependencies'] as const) {
    const deps: Record<string, string> = { dependencies, devDependencies }[field] || {}
    for (const [name, spec] of Object.entries(deps)) {
      if (name === '@dcl/protocol' || !(spec.startsWith('workspace:') || spec.startsWith('link:'))) continue
      const linkedPath = spec.startsWith('link:') ? resolve(path, spec.slice('link:'.length)) : resolve('packages', name)
      it(`pin ${name} in ${field} of ${path}`, () => {
        const { version } = readJson('package.json', linkedPath)
        patchJson('package.json', path, ($) => ({ ...$, [field]: { ...$[field], [name]: version } }))
      })
    }
  }
}

// @dcl/protocol is the link: to library/protocol while developing. A packed manifest names
// the version published from there, which only the caller knows (PROTOCOL_VERSION: the publish
// job's value, or a self-describing one for tarballs that are not published); there is no
// fallback, since the folder's own version is not necessarily on npm.
function pinProtocol(path: string) {
  const packageJson = resolve(path, 'package.json')
  const { dependencies } = JSON.parse(readFileSync(packageJson, 'utf-8'))
  if (!dependencies || !('@dcl/protocol' in dependencies)) return
  it(`pin @dcl/protocol in ${path}`, () => {
    const version = process.env.PROTOCOL_VERSION
    if (!version) {
      throw new Error(
        `PROTOCOL_VERSION is unset: set it to the @dcl/protocol version this build is published with ` +
          `(or e.g. 0.0.0-unpublished.local for tarballs that are not published), so ${path} never packs ` +
          `the link: or a version that is not on npm`
      )
    }
    patchJson('package.json', path, ($) => ({
      ...$,
      dependencies: { ...$.dependencies, '@dcl/protocol': version }
    }))
  })
}

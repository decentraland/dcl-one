import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { stringify } from 'yaml'
import { renderDependencies } from '../scripts/review-signature.mjs'

type Lock = {
  packages: Record<string, Record<string, unknown>>
  snapshots: Record<string, Record<string, Record<string, string>>>
}

// Resolves the render dependencies of a package root that depends on react and builds with vite,
// from a pnpm-lock.yaml only: no node_modules exist, as on a machine of another platform.
async function dependenciesFor(lock: Lock) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'review-signature-'))
  try {
    await writeFile(
      path.join(root, 'pnpm-lock.yaml'),
      stringify({
        lockfileVersion: '9.0',
        importers: {
          '.': {
            devDependencies: {
              react: { specifier: '1.0.0', version: '1.0.0' },
              vite: { specifier: '1.0.0', version: '1.0.0(tsx@1.0.0)' },
              eslint: { specifier: '1.0.0', version: '1.0.0' }
            }
          }
        },
        ...lock
      })
    )
    return await renderDependencies(root, ['react', 'vite'])
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

const integrity = (value: string) => ({ resolution: { integrity: `sha512-${value}` } })

const lock = (overrides: Partial<Record<string, string>> = {}): Lock => ({
  packages: {
    'react@1.0.0': integrity(overrides.react ?? 'react'),
    'vite@1.0.0': {
      ...integrity('vite'),
      peerDependencies: { tsx: '*' },
      peerDependenciesMeta: { tsx: { optional: true } }
    },
    'rolldown@1.0.0': integrity('rolldown'),
    'binding-darwin-arm64@1.0.0': { ...integrity('darwin'), os: ['darwin'], cpu: ['arm64'] },
    'binding-linux-x64@1.0.0': {
      ...integrity(overrides.linuxBinding ?? 'linux'),
      os: ['linux'],
      cpu: ['x64']
    },
    'tsx@1.0.0': integrity(overrides.tsx ?? 'tsx'),
    'eslint@1.0.0': integrity(overrides.eslint ?? 'eslint')
  },
  snapshots: {
    'react@1.0.0': {},
    'vite@1.0.0(tsx@1.0.0)': {
      dependencies: { rolldown: '1.0.0' },
      optionalDependencies: { tsx: '1.0.0' }
    },
    'rolldown@1.0.0': {
      optionalDependencies: { 'binding-darwin-arm64': '1.0.0', 'binding-linux-x64': '1.0.0' }
    },
    'binding-darwin-arm64@1.0.0': {},
    'binding-linux-x64@1.0.0': {},
    'tsx@1.0.0': {},
    'eslint@1.0.0': {}
  }
})

describe('when deciding which dependencies invalidate visual evidence', () => {
  it('should include every platform variant of a binding the lock lists, wherever it runs', async () => {
    const dependencies = await dependenciesFor(lock())
    assert.match(dependencies, /^binding-darwin-arm64@1\.0\.0 sha512-darwin$/m)
    assert.match(dependencies, /^binding-linux-x64@1\.0\.0 sha512-linux$/m)
  })
  it('should change when any platform variant changes, including one not installed here', async () => {
    assert.notEqual(
      await dependenciesFor(lock({ linuxBinding: 'linux-rebuilt' })),
      await dependenciesFor(lock())
    )
  })
  it('should include runtime dependencies', async () => {
    assert.notEqual(
      await dependenciesFor(lock({ react: 'react-2' })),
      await dependenciesFor(lock())
    )
  })
  it('should ignore tooling that does not reach the page', async () => {
    assert.equal(await dependenciesFor(lock({ eslint: 'eslint-2' })), await dependenciesFor(lock()))
  })
  it('should ignore optional peers of the bundler', async () => {
    assert.equal(await dependenciesFor(lock({ tsx: 'tsx-2' })), await dependenciesFor(lock()))
  })
})

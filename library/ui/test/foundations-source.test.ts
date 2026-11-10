import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import source from '../foundations/source.json'

const foundations = path.resolve(import.meta.dirname, '../foundations')
const repository = execFileSync('git', ['rev-parse', '--show-toplevel'], {
  encoding: 'utf8'
}).trim()

describe('when the recorded foundation provenance is checked', () => {
  describe.each(source.output)('and the generated output is $path', (output) => {
    let bytes: Buffer

    beforeEach(async () => {
      bytes = await readFile(path.join(foundations, output.path))
    })

    it('should reject changed output bytes', () => {
      expect(createHash('sha256').update(bytes).digest('hex')).toBe(output.sha256)
    })
  })

  describe.each(source.generatorModules)('and the generator module is $path', (module) => {
    let bytes: Buffer
    let blob: string

    beforeEach(async () => {
      bytes = await readFile(path.join(repository, module.path))
      blob = execFileSync('git', ['hash-object', '--stdin'], {
        input: bytes,
        encoding: 'utf8'
      }).trim()
    })

    it('should reject a changed generator or Git object identity', () => {
      expect({ sha256: createHash('sha256').update(bytes).digest('hex'), blob }).toEqual({
        sha256: module.sha256,
        blob: module.blob
      })
    })
  })

  describe('and the manifest coverage is inspected', () => {
    let generatedPaths: string[]

    beforeEach(async () => {
      generatedPaths = []
      for (const file of await readdir(foundations, { recursive: true })) {
        if (!/\.(tsx?|css)$/.test(file)) continue
        const normalized = file.split(path.sep).join('/')
        if (
          normalized === 'theme.ts' ||
          /^(colors|icons|layout|spacing)\//.test(normalized) ||
          (normalized.startsWith('typography/') &&
            normalized !== 'typography/inter.css' &&
            !normalized.endsWith('.d.ts'))
        ) {
          generatedPaths.push(file.split(path.sep).join('/'))
        }
      }
    })

    it('should account for every generated file exactly once', () => {
      expect(source.output.map((output) => output.path).sort()).toEqual(generatedPaths.sort())
    })

    it('should keep all three generator module identities', () => {
      expect(source.generatorModules).toHaveLength(3)
    })
  })

  describe('and typography CSS module declarations are checked', () => {
    let css: string
    let declarations: string

    beforeEach(async () => {
      css = await readFile(path.join(foundations, 'typography/text-styles.module.css'), 'utf8')
      declarations = await readFile(
        path.join(foundations, 'typography/text-styles.module.css.d.ts'),
        'utf8'
      )
    })

    it('should declare every generated class exactly once', () => {
      const classes = [
        ...new Set([...css.matchAll(/:where\(\.([A-Za-z_][\w-]*)\)/g)].map((match) => match[1]))
      ].sort()
      const types = [...declarations.matchAll(/readonly "([^"]+)": string/g)]
        .map((match) => match[1])
        .sort()
      expect(classes).toHaveLength(32)
      expect(types).toEqual(classes)
    })
  })

  describe('and the capture pin changes', () => {
    let snapshot: { revision: string; digests: Record<string, string> } | undefined

    beforeEach(async () => {
      snapshot = (
        JSON.parse(await readFile(path.join(foundations, '../upstream.lock.json'), 'utf8')) as {
          sources: {
            repository: string
            snapshot: { revision: string; digests: Record<string, string> }
          }[]
        }
      ).sources.find((entry) => entry.repository === source.capture.repository)?.snapshot
    })

    it('should reject a different capture revision or input digest table', () => {
      expect({
        revision: snapshot?.revision,
        inputs: Object.fromEntries(
          Object.keys(source.capture.inputs).map((file) => [file, snapshot?.digests[file]])
        )
      }).toEqual({ revision: source.capture.revision, inputs: source.capture.inputs })
    })

    it('should retain the four captured input families', () => {
      expect(Object.keys(source.capture.inputs).sort()).toEqual([
        'icons.json',
        'paint-styles.json',
        'text-styles.json',
        'variables.json'
      ])
    })
  })
})

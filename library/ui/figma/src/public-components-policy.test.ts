import { beforeEach, describe, expect, it } from 'vitest'
import { publicComponentProblems } from '../public-components-policy.mjs'

type Identity = { sha256: string; indexBlobSha: string }
type Output = Identity & { origin: Record<string, unknown> }
type Context = {
  inspection?: {
    problems: string[]
    outputs: Map<string, Output>
    inputs: Map<string, Identity>
  }
  expected?: Map<string, { sha256: string; origin: Record<string, unknown> }>
  source?: string
  sourceFiles?: Record<string, { sha256: string }>
  transformModules?: Map<string, string>
}
const installer = 'library/ui/figma/src/install-components.ts'
const runtime = 'library/ui/figma/review/src/components/figma-view-state.ts'
const hash = 'a'.repeat(64)
const inputHash = 'b'.repeat(64)
const moduleHash = 'c'.repeat(64)
const blob = 'd'.repeat(40)
let context: Required<Context>
let generated: Record<string, unknown>
let runtimeOrigin: Record<string, unknown>

// Synthetic opaque identities exercise only this seam, not source acquisition or publication.
describe('when verified identities match an independent emitted component plan', () => {
  beforeEach(() => {
    generated = {
      kind: 'generated',
      source: 'synthetic-pinned-source',
      transform: 'public-runtime-imports-v1'
    }
    runtimeOrigin = {
      ...generated,
      kind: 'runtime',
      path: runtime,
      sha256: inputHash
    }
    context = {
      inspection: {
        problems: [],
        outputs: new Map([
          ['Button/Button.tsx', { sha256: hash, indexBlobSha: blob, origin: generated }],
          [
            'runtime/figma-view-state.ts',
            { sha256: hash, indexBlobSha: blob, origin: runtimeOrigin }
          ],
          [
            'promotion.json',
            {
              sha256: hash,
              indexBlobSha: blob,
              origin: {
                kind: 'source',
                repository: 'https://github.com/decentraland/dcl-one'
              }
            }
          ]
        ]),
        inputs: new Map([
          [installer, { sha256: moduleHash, indexBlobSha: blob }],
          [runtime, { sha256: inputHash, indexBlobSha: blob }]
        ])
      },
      expected: new Map([
        ['Button/Button.tsx', { sha256: hash, origin: { ...generated } }],
        ['runtime/figma-view-state.ts', { sha256: hash, origin: { ...runtimeOrigin } }],
        [
          'promotion.json',
          {
            sha256: hash,
            origin: {
              kind: 'source',
              repository: 'https://github.com/decentraland/dcl-one'
            }
          }
        ]
      ]),
      source: 'synthetic-pinned-source',
      sourceFiles: {
        [installer]: { sha256: moduleHash },
        [runtime]: { sha256: inputHash }
      },
      transformModules: new Map([[installer, moduleHash]])
    }
  })

  it('should accept generated and runtime outputs with distinct source and output hashes', () => {
    expect(publicComponentProblems(context)).toEqual([])
  })

  describe.each([
    [
      'absent inspection',
      (c: Required<Context>) => {
        delete (c as Context).inspection
      }
    ],
    [
      'empty inspection',
      (c: Required<Context>) => {
        c.inspection = {} as typeof c.inspection
      }
    ],
    [
      'absent plan',
      (c: Required<Context>) => {
        delete (c as Context).expected
      }
    ],
    [
      'empty plan',
      (c: Required<Context>) => {
        c.expected = new Map()
      }
    ],
    [
      'absent source identity',
      (c: Required<Context>) => {
        c.source = ''
      }
    ],
    [
      'wrong source identity',
      (c: Required<Context>) => {
        c.source = 'different-source'
      }
    ],
    [
      'missing source pins',
      (c: Required<Context>) => {
        c.sourceFiles = {}
      }
    ],
    [
      'missing module identities',
      (c: Required<Context>) => {
        c.transformModules = new Map()
      }
    ],
    [
      'wrong module implementation',
      (c: Required<Context>) => {
        c.transformModules = new Map([['library/ui/figma/src/other.ts', moduleHash]])
      }
    ],
    [
      'changed verified transform',
      (c: Required<Context>) => {
        c.inspection.inputs.set(installer, {
          sha256: hash,
          indexBlobSha: blob
        })
      }
    ],
    [
      'changed locked transform',
      (c: Required<Context>) => {
        c.sourceFiles[installer] = { sha256: hash }
      }
    ],
    [
      'changed verified runtime',
      (c: Required<Context>) => {
        c.inspection.inputs.set(runtime, { sha256: hash, indexBlobSha: blob })
      }
    ],
    [
      'changed locked runtime',
      (c: Required<Context>) => {
        c.sourceFiles[runtime] = { sha256: hash }
      }
    ],
    [
      'missing runtime input',
      (c: Required<Context>) => {
        c.inspection.inputs.delete(runtime)
      }
    ],
    [
      'missing input index identity',
      (c: Required<Context>) => {
        c.inspection.inputs.set(runtime, {
          sha256: inputHash,
          indexBlobSha: ''
        })
      }
    ],
    [
      'missing output',
      (c: Required<Context>) => {
        c.inspection.outputs.delete('Button/Button.tsx')
      }
    ],
    [
      'extra output',
      (c: Required<Context>) => {
        c.inspection.outputs.set('extra.tsx', {
          sha256: hash,
          indexBlobSha: blob,
          origin: generated
        })
      }
    ],
    [
      'changed output digest',
      (c: Required<Context>) => {
        c.inspection.outputs.get('Button/Button.tsx')!.sha256 = inputHash
      }
    ],
    [
      'changed emitted digest',
      (c: Required<Context>) => {
        c.expected.get('Button/Button.tsx')!.sha256 = inputHash
      }
    ],
    [
      'missing emitted digest',
      (c: Required<Context>) => {
        c.expected.get('Button/Button.tsx')!.sha256 = ''
      }
    ],
    [
      'missing output index identity',
      (c: Required<Context>) => {
        c.inspection.outputs.get('Button/Button.tsx')!.indexBlobSha = ''
      }
    ],
    [
      'relabelled generated claim',
      (c: Required<Context>) => {
        c.inspection.outputs.get('Button/Button.tsx')!.origin = {
          kind: 'source',
          repository: 'https://github.com/decentraland/dcl-one'
        }
      }
    ],
    [
      'relabelled emitted generated file',
      (c: Required<Context>) => {
        c.expected.get('Button/Button.tsx')!.origin = {
          kind: 'source',
          repository: 'https://github.com/decentraland/dcl-one'
        }
      }
    ],
    [
      'unverified LFS pointer reported by inspector',
      (c: Required<Context>) => {
        c.inspection.problems.push('unmaterialized public file')
      }
    ],
    [
      'prototype source table',
      (c: Required<Context>) => {
        c.sourceFiles = Object.create(c.sourceFiles)
      }
    ]
  ])('and there is %s', (_label, change) => {
    beforeEach(() => {
      change(context)
    })
    it('should reject the incomplete or conflicting identities', () => {
      expect(publicComponentProblems(context).length).toBeGreaterThan(0)
    })
  })

  describe.each([
    ['unknown kind', { kind: 'npm' }],
    ['ambiguous metadata', { unexpected: true }],
    ['wrong source', { source: 'wrong-source' }],
    ['wrong transform label', { transform: 'other-transform' }],
    ['a stale per-output module map', { modules: { [installer]: moduleHash } }]
  ])('and the output claims %s', (_label, override) => {
    beforeEach(() => {
      context.inspection.outputs.get('Button/Button.tsx')!.origin = {
        ...generated,
        ...override
      }
    })
    it('should reject the output origin', () => {
      expect(publicComponentProblems(context)).toContain(
        'component origin differs: Button/Button.tsx'
      )
    })
  })

  describe.each([
    '../figma-view-state.ts',
    '/figma-view-state.ts',
    'constructor',
    '__proto__',
    'prototype',
    'library/ui/figma/review/src/components/../components/figma-view-state.ts',
    'library/ui/figma/review/src/components/StateMatrix.tsx',
    'library/ui/figma/review/src/components/figma-view-state.ts/'
  ])('and runtime claims the unsafe or unlocked key %s', (key) => {
    beforeEach(() => {
      context.inspection.outputs.get('runtime/figma-view-state.ts')!.origin = {
        ...runtimeOrigin,
        path: key
      }
    })
    it('should reject the runtime source key', () => {
      expect(publicComponentProblems(context)).toContain(
        'component origin differs: runtime/figma-view-state.ts'
      )
    })
  })

  describe.each([
    '../Button.tsx',
    '/Button.tsx',
    '__proto__/Button.tsx',
    'constructor',
    'Button//Button.tsx'
  ])('and the plan contains unsafe output %s', (key) => {
    beforeEach(() => {
      context.expected.set(key, { sha256: hash, origin: generated })
      context.inspection.outputs.set(key, { sha256: hash, indexBlobSha: blob, origin: generated })
    })
    it('should reject the unsafe plan path', () => {
      expect(publicComponentProblems(context)).toContain('unsafe component plan path: ' + key)
    })
  })
  describe.each(['notes.figma.ts', 'Other/promotion.json', 'Button/Button.test.tsx'])(
    'and a maintained claim uses unallowlisted file %s',
    (file) => {
      beforeEach(() => {
        context.expected.set(file, {
          sha256: hash,
          origin: { kind: 'source', repository: 'https://github.com/decentraland/dcl-one' }
        })
        context.inspection.outputs.set(file, {
          sha256: hash,
          indexBlobSha: blob,
          origin: { kind: 'source', repository: 'https://github.com/decentraland/dcl-one' }
        })
      })
      it('should reject the maintained file regardless of suffix', () => {
        expect(publicComponentProblems(context)).toContain(
          'invalid independent component origin: ' + file
        )
      })
    }
  )
  describe.each([
    'figma-module-runtime.tsx',
    'figma-module-runtime.module.css',
    'figma-interactions.tsx'
  ])('and runtime uses allowed file %s', (file) => {
    beforeEach(() => {
      context.sourceFiles['library/ui/figma/review/src/components/' + file] = { sha256: inputHash }
      context.inspection.inputs.set('library/ui/figma/review/src/components/' + file, {
        sha256: inputHash,
        indexBlobSha: blob
      })
      context.expected.set('runtime/' + file, {
        sha256: hash,
        origin: { ...runtimeOrigin, path: 'library/ui/figma/review/src/components/' + file }
      })
      context.inspection.outputs.set('runtime/' + file, {
        sha256: hash,
        indexBlobSha: blob,
        origin: { ...runtimeOrigin, path: 'library/ui/figma/review/src/components/' + file }
      })
    })
    it('should accept the independently bound runtime output', () => {
      expect(publicComponentProblems(context)).toEqual([])
    })
  })
  describe('and a generator module is independently bound', () => {
    beforeEach(() => {
      context.transformModules.set('library/ui/figma/src/generators/components.ts', moduleHash)
      context.sourceFiles['library/ui/figma/src/generators/components.ts'] = {
        sha256: moduleHash
      }
      context.inspection.inputs.set('library/ui/figma/src/generators/components.ts', {
        sha256: moduleHash,
        indexBlobSha: blob
      })
    })
    it('should accept the additional verified generator binding', () => {
      expect(publicComponentProblems(context)).toEqual([])
    })
    it('should reject it when its verified bytes differ from the single module pin', () => {
      context.inspection.inputs.set('library/ui/figma/src/generators/components.ts', {
        sha256: hash,
        indexBlobSha: blob
      })
      expect(publicComponentProblems(context)).toContain(
        'missing or mismatched component transform identities'
      )
    })
  })
})

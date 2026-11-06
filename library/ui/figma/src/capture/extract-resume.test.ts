import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

// The extractor runs inside Figma and returns one part per run when VARIANT_LIMIT is set. These tests run its
// whole script against a small fake file and follow `resume` the way a caller does, so a page or a variant the
// loop forgets shows up as a part that is never returned.
const script = readFileSync(
  fileURLToPath(new URL('../../capture/extract-components.figma.js', import.meta.url)),
  'utf8'
)
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor as new (
  ...args: string[]
) => (figma: unknown) => Promise<Result>

type Resume = { page: number; component: number; variant?: number }
type Result = {
  pages: { page: { id: string }; components: { id: string; variantRange?: { start: number } }[] }[]
  resume: Resume | null
}

const variant = (id: string, name: string) => ({
  id,
  type: 'COMPONENT',
  name,
  width: 10,
  height: 10,
  children: []
})

// A component set with `others` non-base variants.
function componentSet(id: string, others: number) {
  const node: Record<string, unknown> = {
    id,
    type: 'COMPONENT_SET',
    name: `Set ${id}`,
    x: 0,
    y: 0,
    componentPropertyDefinitions: {},
    parent: { type: 'PAGE', name: 'page' }
  }
  node.children = Array.from({ length: others + 1 }, (_, index) => ({
    ...variant(`${id}:${index}`, `state=${index}`),
    parent: node
  }))
  return node
}

function page(id: string, sets: Record<string, unknown>[]) {
  return {
    id,
    type: 'PAGE',
    name: `Page ${id}`,
    children: [
      {
        name: 'Sync status',
        type: 'INSTANCE',
        componentProperties: { ready: { type: 'VARIANT', value: 'true' } },
        getMainComponentAsync: async () => ({ parent: { id: '15973:15' } })
      }
    ],
    findAllWithCriteria: ({ types }: { types: string[] }) => (types.includes('TEXT') ? [] : sets)
  }
}

async function run(
  pages: ReturnType<typeof page>[],
  limit: number,
  from: { page: number; component: number; variant: number }
) {
  const source = script
    .replace(
      'const PAGE_IDS = []',
      `const PAGE_IDS = ${JSON.stringify(pages.map((item) => item.id))}`
    )
    .replace('const START_PAGE = 0', `const START_PAGE = ${from.page}`)
    .replace('const START_COMPONENT = 0', `const START_COMPONENT = ${from.component}`)
    .replace('const VARIANT_START = 0', `const VARIANT_START = ${from.variant}`)
    .replace('const VARIANT_LIMIT = 0', `const VARIANT_LIMIT = ${limit}`)
  const byId = new Map(pages.map((item) => [item.id, item]))
  const figma = {
    mixed: Symbol('mixed'),
    getNodeByIdAsync: async (id: string) => byId.get(id) ?? null,
    setCurrentPageAsync: async () => {}
  }
  return new AsyncFunction('figma', source)(figma)
}

// Every part a caller receives when it re-runs with the returned resume until it is null.
async function parts(pages: ReturnType<typeof page>[], limit: number) {
  const received: { page: string; set: string; start: number }[] = []
  const resumes: (Resume | null)[] = []
  let from = { page: 0, component: 0, variant: 0 }
  for (let guard = 0; guard < 50; guard++) {
    const result = await run(pages, limit, from)
    for (const captured of result.pages)
      for (const component of captured.components)
        received.push({
          page: captured.page.id,
          set: component.id,
          start: component.variantRange?.start ?? 0
        })
    resumes.push(result.resume)
    if (!result.resume) return { received, resumes }
    from = {
      page: result.resume.page,
      component: result.resume.component,
      variant: result.resume.variant ?? from.variant
    }
  }
  throw new Error('the resume never ended')
}

describe('when a job is captured in parts of VARIANT_LIMIT variants', () => {
  describe('and the first page ends with a component that fits in one part', () => {
    it('should continue with the next page instead of ending the job', async () => {
      const pages = [page('1:1', [componentSet('10:1', 2)]), page('1:2', [componentSet('20:1', 1)])]

      const { received, resumes } = await parts(pages, 2)

      expect(received).toEqual([
        { page: '1:1', set: '10:1', start: 0 },
        { page: '1:2', set: '20:1', start: 0 }
      ])
      expect(resumes).toEqual([{ page: 1, component: 0, variant: 0 }, null])
    })
  })

  describe('and a component has more variants than one part holds', () => {
    it('should continue the same component from the next variant, then move on', async () => {
      const pages = [
        page('1:1', [componentSet('10:1', 5), componentSet('10:2', 1)]),
        page('1:2', [componentSet('20:1', 3)])
      ]

      const { received } = await parts(pages, 2)

      expect(received).toEqual([
        { page: '1:1', set: '10:1', start: 0 },
        { page: '1:1', set: '10:1', start: 2 },
        { page: '1:1', set: '10:1', start: 4 },
        { page: '1:1', set: '10:2', start: 0 },
        { page: '1:2', set: '20:1', start: 0 },
        { page: '1:2', set: '20:1', start: 2 }
      ])
    })
  })

  describe('and a component has exactly VARIANT_LIMIT variants, or one more', () => {
    it('should capture the exact fit in one part and give the extra variant its own part', async () => {
      const pages = [page('1:1', [componentSet('10:1', 3), componentSet('10:2', 4)])]

      const { received, resumes } = await parts(pages, 3)

      expect(received).toEqual([
        { page: '1:1', set: '10:1', start: 0 },
        { page: '1:1', set: '10:2', start: 0 },
        { page: '1:1', set: '10:2', start: 3 }
      ])
      expect(resumes).toEqual([
        { page: 0, component: 1, variant: 0 },
        { page: 0, component: 1, variant: 3 },
        null
      ])
    })
  })

  describe('and the job is a single page', () => {
    it('should end with a null resume after its last part', async () => {
      const { resumes } = await parts([page('1:1', [componentSet('10:1', 1)])], 5)

      expect(resumes).toEqual([null])
    })
  })

  describe('and three pages follow one another', () => {
    it('should reach every page, in order', async () => {
      const pages = [1, 2, 3].map((n) => page(`1:${n}`, [componentSet(`${n}0:1`, 1)]))

      const { received } = await parts(pages, 4)

      expect(received.map((item) => item.page)).toEqual(['1:1', '1:2', '1:3'])
    })
  })
})

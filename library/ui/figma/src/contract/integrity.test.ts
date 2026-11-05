import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { createRuntime } from '../runtime.js'
import { checkMappings, resolveComponent } from './resolve.js'

const projectRoot = path.resolve(import.meta.dirname, '../..')

async function realContext() {
  const runtime = await createRuntime({
    cwd: projectRoot,
    env: {},
    configPath: 'test/fixtures/figma.config.json'
  })
  return runtime.resolveContext()
}

// The resolver runs against frozen fixtures; `check-mappings` verifies the live library against the live capture.
describe('component library mappings', () => {
  describe('when checked against the fixture library and capture', () => {
    it('should have no errors and no drift warnings', async () => {
      const results = await checkMappings(await realContext())

      expect(results.length).toBeGreaterThan(0)
      expect(
        results.flatMap((r) =>
          r.diagnostics
            .filter((d) => d.severity === 'error' || d.code === 'figma-drift')
            .map((d) => `${r.file}: ${d.message}`)
        )
      ).toEqual([])
    })
  })

  describe('when resolving the real Figma Button for every state the design defines', () => {
    it.each([
      ['Primary', 'No', 'No', '<Button variant="primary">Label</Button>'],
      ['Primary', 'No', 'Yes', '<Button variant="primary" disabled>Label</Button>'],
      ['Secondary', 'Yes', 'No', '<Button variant="secondary">Label</Button>'],
      ['Terciary', 'No', 'No', '<Button variant="tertiary">Label</Button>'],
      ['Quaternary', 'Yes', 'No', '<Button variant="quaternary">Label</Button>']
    ])(
      'should map Property 1=%s, Hover=%s, Blocked=%s to %s',
      async (kind, hover, blocked, usage) => {
        const result = await resolveComponent(await realContext(), {
          nodeId: '14113:6739',
          variantProperties: { 'Property 1': kind, Hover: hover, Blocked: blocked }
        })

        expect(result.ok && result.resolved.usage).toBe(usage)
      }
    )
  })

  describe('when the design has no Blocked variant for the requested kind', () => {
    it('should refuse the combination instead of inventing a disabled secondary button', async () => {
      const result = await resolveComponent(await realContext(), {
        nodeId: '14113:6739',
        variantProperties: { 'Property 1': 'Secondary', Blocked: 'Yes' }
      })

      expect(!result.ok && result.errors.map((e) => e.code)).toEqual(['variant-not-in-figma'])
    })
  })

  describe('when resolving the real Figma Shop Modals with scrolling content', () => {
    it('should map it to the library Modal with the title read from the design', async () => {
      const result = await resolveComponent(await realContext(), { nodeId: '15912:1382' })

      expect(result.ok && result.resolved.usage).toBe(
        '<Modal title="Buy Item" onClose={onClose} actions={actions}>{content}</Modal>'
      )
    })
  })

  describe('when a Figma component that is not in the library is requested', () => {
    it('should report mapping-missing for the icon variant of the button', async () => {
      const result = await resolveComponent(await realContext(), { nodeId: '14113:6758' })

      expect(!result.ok && result.errors.map((e) => e.code)).toEqual(['mapping-missing'])
    })
  })
})

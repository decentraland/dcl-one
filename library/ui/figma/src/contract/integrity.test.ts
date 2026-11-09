import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { createRuntime } from '../runtime.js'
import { checkMappings, resolveComponent } from './resolve.js'

const projectRoot = path.resolve(import.meta.dirname, '../..')

async function fixtureContext() {
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
      const results = await checkMappings(await fixtureContext())

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

  describe('when resolving the synthetic Button for every state the design defines', () => {
    it.each([
      ['Primary', 'No', 'No', '<Button variant="primary">Label</Button>'],
      ['Primary', 'No', 'Yes', '<Button variant="primary" disabled>Label</Button>'],
      ['Secondary', 'Yes', 'No', '<Button variant="secondary">Label</Button>'],
      ['Terciary', 'No', 'No', '<Button variant="tertiary">Label</Button>'],
      ['Quaternary', 'Yes', 'No', '<Button variant="quaternary">Label</Button>']
    ])(
      'should map Property 1=%s, Hover=%s, Blocked=%s to %s',
      async (kind, hover, blocked, usage) => {
        const result = await resolveComponent(await fixtureContext(), {
          nodeId: '20:1',
          variantProperties: { 'Property 1': kind, Hover: hover, Blocked: blocked }
        })

        expect(result.ok && result.resolved.usage).toBe(usage)
      }
    )
  })

  describe('when the design has no Blocked variant for the requested kind', () => {
    it('should refuse the combination instead of inventing a disabled secondary button', async () => {
      const result = await resolveComponent(await fixtureContext(), {
        nodeId: '20:1',
        variantProperties: { 'Property 1': 'Secondary', Blocked: 'Yes' }
      })

      expect(!result.ok && result.errors.map((e) => e.code)).toEqual(['variant-not-in-figma'])
    })
  })

  describe('when resolving the synthetic Dialog variant with scrolling content', () => {
    it('should map it to the library Modal with the title read from the design', async () => {
      const result = await resolveComponent(await fixtureContext(), { nodeId: '30:3' })

      expect(result.ok && result.resolved.usage).toBe(
        '<Modal title="Dialog Title" onClose={onClose} actions={actions}>{content}</Modal>'
      )
      expect(result.ok && result.resolved.usageFor).toEqual({
        variantProperties: { Size: 'Default', Scroll: 'Yes' },
        basis: 'requested'
      })
    })
  })

  describe('when a Figma component that is not in the library is requested', () => {
    it('should report mapping-missing for the split button', async () => {
      const result = await resolveComponent(await fixtureContext(), { nodeId: '20:50' })

      expect(!result.ok && result.errors.map((e) => e.code)).toEqual(['mapping-missing'])
    })
  })
})

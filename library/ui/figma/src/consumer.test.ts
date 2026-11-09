import { mkdtemp, rm, symlink, mkdir, readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { Client } from '@modelcontextprotocol/sdk/client/index.js'
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'

const projectRoot = path.resolve(import.meta.dirname, '..')
const libraryRoot = path.resolve(projectRoot, 'test/fixtures/library')
const libraryVersion = JSON.parse(await readFile(path.join(libraryRoot, 'package.json'), 'utf8'))
  .version as string

type TextResult = { isError?: boolean; content: { type: string; text: string }[] }

// A consuming project is a directory with the library installed and no Figma config of its own. The server
// runs with that directory as its working directory, exactly as an MCP client would start it.
async function startServer(cwd: string, args: string[], env: Record<string, string> = {}) {
  const client = new Client({ name: 'consumer-test', version: '0.0.0' })
  await client.connect(
    new StdioClientTransport({
      command: process.execPath,
      args: [
        '--import',
        import.meta.resolve('tsx'),
        path.join(projectRoot, 'src/server.ts'),
        ...args
      ],
      cwd,
      env: { PATH: process.env.PATH ?? '', ...env },
      stderr: 'pipe'
    })
  )
  const call = async (name: string, input: Record<string, unknown> = {}) => {
    const result = (await client.callTool({ name, arguments: input })) as TextResult
    return { isError: result.isError ?? false, body: JSON.parse(result.content[0]!.text) }
  }
  return { client, call }
}

describe('figma from a consuming project', () => {
  let consumer: string

  beforeAll(async () => {
    consumer = await mkdtemp(path.join(tmpdir(), 'dcl-figma-consumer-'))
    await mkdir(path.join(consumer, 'node_modules/@dcl'), { recursive: true })
    await symlink(libraryRoot, path.join(consumer, 'node_modules/@dcl/one-ui'), 'dir')
  })

  afterAll(async () => {
    await rm(consumer, { recursive: true, force: true })
  })

  describe('when started with only a mapping root (no Figma config)', () => {
    it('should run in resolve-only mode and resolve the Button from the installed package', async () => {
      const { client, call } = await startServer(consumer, [
        '--mapping-root',
        'node_modules/@dcl/one-ui/components'
      ])

      const { tools } = await client.listTools()
      const resolved = await call('figma_resolve_component', {
        nodeId: '20:1',
        variantProperties: { 'Property 1': 'Terciary' }
      })
      await client.close()

      expect(tools.map((t) => t.name).sort()).toEqual([
        'figma_check_mappings',
        'figma_resolve_component'
      ])
      expect(resolved.isError).toBe(false)
      expect(resolved.body.resolved.usage).toBe('<Button variant="tertiary">Label</Button>')
      expect(resolved.body.resolved.provenance.figma.identity).toBe('mapping-only')
      expect(resolved.body.warnings.map((w: { code: string }) => w.code)).toContain(
        'figma-identity-unverified'
      )
    }, 30_000)
  })

  describe('when started with the maintainer config and the mapping root from the environment', () => {
    it('should expose every tool and verify the Figma identity against the capture', async () => {
      const { client, call } = await startServer(
        consumer,
        ['--config', path.join(projectRoot, 'test/fixtures/figma.config.json')],
        { DCL_FIGMA_MAPPING_ROOT: 'node_modules/@dcl/one-ui/components' }
      )

      const { tools } = await client.listTools()
      const resolved = await call('figma_resolve_component', { name: 'Synthetic Button' })
      await client.close()

      expect(tools).toHaveLength(9)
      expect(resolved.body.resolved.provenance.figma.identity).toBe('verified-against-capture')
      expect(resolved.body.resolved.provenance.library).toEqual({
        name: '@dcl/one-ui',
        version: libraryVersion
      })
    }, 30_000)
  })

  describe('when started with no mapping source at all', () => {
    it('should fail the call with mapping-root-not-configured instead of guessing a location', async () => {
      const { client, call } = await startServer(consumer, [])

      const resolved = await call('figma_resolve_component', { nodeId: '20:1' })
      await client.close()

      expect(resolved.isError).toBe(true)
      expect(resolved.body.errors[0].code).toBe('mapping-root-not-configured')
    }, 30_000)
  })

  describe('when the library range required by the consumer is not satisfied', () => {
    it('should report contract-incompatible through the server', async () => {
      const { client, call } = await startServer(consumer, [
        '--mapping-root',
        'node_modules/@dcl/one-ui/components',
        '--library-range',
        '^9.0.0'
      ])

      const resolved = await call('figma_resolve_component', { nodeId: '20:1' })
      await client.close()

      expect(resolved.isError).toBe(true)
      expect(resolved.body.errors[0].details.reason).toBe('library-version')
    }, 30_000)
  })
})

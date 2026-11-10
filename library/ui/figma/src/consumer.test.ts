import { mkdtemp, rm, symlink, mkdir, readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { execFile } from 'node:child_process'
import path from 'node:path'
import { promisify } from 'node:util'
import { Client } from '@modelcontextprotocol/sdk/client/index.js'
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'

const run = promisify(execFile)
const projectRoot = path.resolve(import.meta.dirname, '..')
const libraryRoot = path.resolve(projectRoot, 'test/fixtures/library')
const libraryVersion = JSON.parse(await readFile(path.join(libraryRoot, 'package.json'), 'utf8'))
  .version as string

const cli = (args: string[]) =>
  run(process.execPath, ['--import', import.meta.resolve('tsx'), 'src/cli.ts', ...args], {
    cwd: projectRoot,
    env: { PATH: process.env.PATH ?? '' }
  })

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

      expect(tools).toHaveLength(10)
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

  describe('when started with --mapping-only in a checkout that has a Figma config', () => {
    const input = {
      nodeId: '20:1',
      variantProperties: { 'Property 1': 'Terciary', Blocked: 'Yes' },
      texts: { Label: 'Go <now>' }
    }
    let mcp: Record<string, unknown>
    let tools: string[]
    beforeAll(async () => {
      // The config would load the capture; --mapping-only must ignore it and register only the contract tools.
      const { client, call } = await startServer(
        projectRoot,
        ['--mapping-only', '--mapping-root', path.join(libraryRoot, 'components')],
        { DCL_FIGMA_CONFIG: 'test/fixtures/figma.config.json' }
      )
      tools = (await client.listTools()).tools.map((t) => t.name).sort()
      mcp = (await call('figma_resolve_component', input)).body
      await client.close()
    }, 30_000)

    it('should register exactly the resolve and check tools', () => {
      expect(tools).toEqual(['figma_check_mappings', 'figma_resolve_component'])
    })

    it('should return the same result from the CLI, texts included', async () => {
      const { stdout } = await cli([
        'resolve',
        '--mapping-only',
        '--mapping-root',
        path.join(libraryRoot, 'components'),
        '--node-id',
        input.nodeId,
        '--variant',
        'Property 1=Terciary',
        '--variant',
        'Blocked=Yes',
        '--text',
        'Label=Go <now>'
      ])
      expect(JSON.parse(stdout)).toEqual(mcp)
      expect(mcp).toMatchObject({
        ok: true,
        resolved: {
          usage: '<Button variant="tertiary" disabled>Go &lt;now&gt;</Button>',
          provenance: { figma: { identity: 'mapping-only' } }
        }
      })
    }, 30_000)

    it('should name no Figma file other than the placeholder', () => {
      const text = JSON.stringify(mcp)
      expect(text).not.toMatch(/figma\.com/i)
      expect(
        (mcp as { resolved: { provenance: { figma: { url: string } } } }).resolved.provenance.figma
          .url
      ).toMatch(/^<DCL_FIGMA_FILE>/)
    })
  })

  describe('when the CLI checks mappings without a capture', () => {
    it('should say the capture checks did not run', async () => {
      const { stdout } = await cli([
        'check-mappings',
        '--mapping-only',
        '--mapping-root',
        path.join(libraryRoot, 'components')
      ])

      expect(JSON.parse(stdout)).toMatchObject({
        ok: true,
        captureChecked: false,
        warnings: [expect.objectContaining({ code: 'capture-unchecked', audience: 'maintainer' })]
      })
    }, 30_000)

    it('should fail without a mapping source instead of passing an empty check', async () => {
      await expect(
        cli(['check-mappings', '--mapping-only', '--mapping-root', tmpdir()])
      ).rejects.toMatchObject({
        code: 1,
        stdout: expect.stringContaining('mapping-root-not-configured')
      })
    }, 30_000)
  })

  describe('when a malformed flag meets a configuration that cannot load', () => {
    it('should report the flag before touching the runtime', async () => {
      await expect(
        cli(['resolve', '--config', 'missing.json', '--node-id', '20:1', '--text', 'Label'])
      ).rejects.toMatchObject({
        code: 1,
        stderr: expect.stringContaining('--text takes "Name=Value"')
      })
    }, 30_000)
  })

  describe('when the CLI receives malformed resolve flags', () => {
    it.each([
      [['--text', 'Label'], '--text takes "Name=Value"'],
      [['--text', '=Go'], '--text takes "Name=Value"'],
      [['--text', 'Label=a', '--text', 'Label=b'], '--text names "Label" twice'],
      [['--variant', 'Blocked'], '--variant takes "Name=Value"'],
      [['--text', '__proto__=a', '--text', '__proto__=b'], '--text names "__proto__" twice'],
      [['--config', 'missing.json'], 'Figma config not found']
    ])(
      'should refuse %j without resolving',
      async (flags, message) => {
        const failure = cli(['resolve', '--mapping-only', '--node-id', '20:1', ...flags])
        await expect(failure).rejects.toMatchObject({
          code: 1,
          stderr: expect.stringContaining(message)
        })
      },
      30_000
    )

    it.each([
      [
        ['check-mappings', '--mapping-only', '--text', 'Label=Go'],
        '--text applies only to resolve'
      ],
      [
        ['review-manifest', '--mapping-only'],
        '--mapping-only applies only to resolve and check-mappings'
      ]
    ])(
      'should refuse %j',
      async (args, message) => {
        await expect(cli(args)).rejects.toMatchObject({
          code: 1,
          stderr: expect.stringContaining(message)
        })
      },
      30_000
    )
  })
})

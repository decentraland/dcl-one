#!/usr/bin/env node
import { parseArgs } from 'node:util'
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import { createRuntime } from './runtime.js'
import { errorText } from './server-errors.js'
import { createContractTools } from './tools/contract.js'
import { createTools } from './tools/index.js'

const isFailure = (result: unknown) =>
  typeof result === 'object' && result !== null && 'ok' in result && result.ok === false

async function main(): Promise<void> {
  const { values } = parseArgs({
    options: {
      config: { type: 'string' },
      'mapping-root': { type: 'string' },
      'library-range': { type: 'string' }
    }
  })
  const runtime = await createRuntime({
    cwd: process.cwd(),
    env: process.env,
    configPath: values.config,
    mappingRoot: values['mapping-root'],
    libraryRange: values['library-range']
  })
  const server = new McpServer({ name: 'figma', version: '0.1.0' })

  // Without a Figma config the server runs in resolve-only mode, which is what a consuming project needs.
  const definitions = [
    ...(runtime.app ? createTools(runtime.app) : []),
    ...createContractTools(runtime)
  ]
  for (const definition of definitions) {
    server.registerTool(
      definition.name,
      { description: definition.description, inputSchema: definition.inputSchema },
      async (input: unknown) => {
        try {
          const result = await definition.handler(input as never)
          return {
            isError: isFailure(result),
            content: [{ type: 'text' as const, text: JSON.stringify(result, null, 2) }]
          }
        } catch (error) {
          return { isError: true, content: [{ type: 'text' as const, text: errorText(error) }] }
        }
      }
    )
  }

  await server.connect(new StdioServerTransport())
}

main().catch((error: unknown) => {
  process.stderr.write(`${errorText(error)}\n`)
  process.exit(1)
})

import path from 'node:path'
import { createElement } from 'react'
import { render, screen } from '@testing-library/react'
import ts from 'typescript'
import { beforeEach, describe, expect, it } from 'vitest'
import { CheckboxLabel } from './CheckboxLabel'

type Resolve = (
  context: { mappingRoot: string; figmaSets: [] },
  input: { name: string; variantProperties: Record<string, string>; texts: Record<string, string> }
) => Promise<{ ok: true; resolved: { usage: string } } | { ok: false; errors: { code: string }[] }>

// The Figma tooling is Node-only and outside this project's type graph, so it is loaded by path.
const resolverPath = path.resolve(import.meta.dirname, '../../figma/src/contract/resolve.ts')
const mappingRoot = path.resolve(import.meta.dirname, '..')

async function usageFor(position: 'End' | 'Start') {
  const { resolveComponent } = (await import(/* @vite-ignore */ resolverPath)) as {
    resolveComponent: Resolve
  }
  const result = await resolveComponent(
    { mappingRoot, figmaSets: [] },
    {
      name: 'checkbox-label',
      variantProperties: { 'Property 1': position },
      texts: { 'Open Debug Console': 'Open Debug Console' }
    }
  )
  if (!result.ok) throw new Error(result.errors.map((error) => error.code).join(', '))
  return result.resolved.usage
}

// Renders the resolver's usage as written, with the caller's help text in scope.
function renderUsage(usage: string, help: string) {
  const code = ts.transpileModule(`const element = (${usage})`, {
    compilerOptions: { jsx: ts.JsxEmit.React, jsxFactory: 'createElement' }
  }).outputText
  const element = new Function('createElement', 'CheckboxLabel', 'help', `${code}\nreturn element`)(
    createElement,
    CheckboxLabel,
    help
  )
  return render(element)
}

describe('when a design checkbox-label resolves through its mapping', () => {
  describe.each([
    ['End', ['box', 'text']],
    ['Start', ['text', 'box']]
  ] as const)('and the box sits at the %s position', (position, order) => {
    let usage: string
    let container: HTMLElement

    beforeEach(async () => {
      usage = await usageFor(position)
      container = renderUsage(usage, 'Shows engine logs in a panel.').container
    })

    it('should keep the help affordance in the resolved usage and in its rendered order', () => {
      expect(usage).toContain('help={help}')
      const box = screen.getByRole('checkbox', { name: 'Open Debug Console' })
      expect(box).toHaveAccessibleDescription('Shows engine logs in a panel.')
      expect(screen.getByRole('button', { name: 'More information' })).toBeInTheDocument()
      const parts = Array.from(container.firstElementChild!.children).map((child) =>
        child.querySelector('input') ? 'box' : 'text'
      )
      expect(parts).toEqual(order)
    })
  })
})

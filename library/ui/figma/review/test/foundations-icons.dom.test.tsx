import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import data from '../src/review/catalog.json'
import * as generatedIcons from '../src/generated/icons'
import { Foundations } from '../src/review/Foundations'

afterEach(() => cleanup())

// Each icon tile names one source variant, so it draws that variant's art: an active=true tile shows the active
// state, and an icon without an active state is never handed the prop.
const tile = (container: HTMLElement, nodeId: string) =>
  container.querySelector(`[title="Figma node ${nodeId}"] svg`)!.outerHTML
const sourceVariant = (name: string, variant: string) =>
  data.foundations.icons.find((icon) => icon.name === name && icon.variant === variant)!

describe('the Foundations icon tiles', () => {
  it('should draw the active art on an active=true tile and the idle art on its active=false tile', () => {
    const { container } = render(<Foundations section="icons" />)
    const on = sourceVariant('navbar/wallet', 'active=true')
    const off = sourceVariant('navbar/wallet', 'active=false')
    const size = Math.min(Math.max(on.width, on.height), 64)
    const expected = (active: boolean) =>
      render(<generatedIcons.NavbarWallet size={size} active={active} aria-hidden />).container
        .firstElementChild!.outerHTML

    expect(tile(container, on.nodeId)).not.toBe(tile(container, off.nodeId))
    expect(tile(container, on.nodeId)).toBe(expected(true))
    expect(tile(container, off.nodeId)).toBe(expected(false))
  })

  it('should not pass active to an icon without an active state', () => {
    const { container } = render(<Foundations section="icons" />)
    const single = data.foundations.icons.find((icon) => !icon.variant)!

    expect(
      container.querySelector(`[title="Figma node ${single.nodeId}"] svg`)!.getAttribute('active')
    ).toBeNull()
  })
})

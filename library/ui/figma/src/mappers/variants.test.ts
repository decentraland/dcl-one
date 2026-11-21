import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import { snapshotDir } from '../../test/snapshot.js'
import { expandVariants } from '../capture/expand-variants.js'
import { rawComponentPageSchema, type RawComponent } from '../ir/raw-components.js'
import { mapVariants, parseOption, splitWords, viewKey, type VariantOptions } from './variants.js'
import { encodePageFileName } from '../capture/page-file-name.js'

const wordAliases = { terciary: 'tertiary' }
const defaults: VariantOptions = { wordAliases }

async function rawComponent(page: string, name: string): Promise<RawComponent> {
  const raw = await readFile(path.join(snapshotDir, 'components', encodePageFileName(page)), 'utf8')
  const found = rawComponentPageSchema
    .parse(JSON.parse(raw))
    .components.find((c) => c.name === name)
  if (!found) throw new Error(`${name} not in ${page}`)
  return found
}

const mapped = async (page: string, name: string, options: Partial<VariantOptions> = {}) => {
  const component = await rawComponent(page, name)
  return mapVariants(component, expandVariants(component), { ...defaults, ...options })
}

describe('splitWords', () => {
  it.each([
    ['camel case', 'SelectedHover', ['selected', 'hover']],
    ['separators', 'Primary-Hover', ['primary', 'hover']],
    ['digits', '100%', ['100']]
  ])('should split %s into lowercase words', (_label, input, expected) => {
    expect(splitWords(input)).toEqual(expected)
  })
})

describe('parseOption', () => {
  describe('when an option mixes a value and interaction states', () => {
    it('should separate the state flags from the remaining value', () => {
      const parsed = parseOption('Terciary-Hover', defaults)

      expect(parsed.remainder).toBe('tertiary')
      expect([...parsed.flags]).toEqual(['hover'])
    })
  })

  describe('when the option only names the base state', () => {
    it('should have no flags and no remainder', () => {
      expect(parseOption('Idle', defaults)).toEqual({ flags: new Set(), remainder: '' })
    })
  })

  describe('when a state word is not active for the component', () => {
    it('should keep it as part of the value', () => {
      const parsed = parseOption('Blocked', { ...defaults, stateWords: ['hover'] })

      expect(parsed.remainder).toBe('blocked')
      expect(parsed.flags.size).toBe(0)
    })
  })
})

describe('mapVariants', () => {
  describe('when mapping the main Button', () => {
    it('should expose variant and disabled props and hover as an interaction', async () => {
      const result = await mapped('Demo Controls', 'Synthetic Button')

      expect(result.props).toEqual([
        {
          name: 'variant',
          kind: 'enum',
          values: ['primary', 'secondary', 'quaternary', 'tertiary'],
          default: 'primary',
          source: 'Property 1'
        },
        {
          name: 'disabled',
          kind: 'boolean',
          values: ['false', 'true'],
          default: false,
          source: 'disabled'
        }
      ])
      const hover = result.variants.filter((variant) => variant.interaction === 'hover')
      expect(hover.map(viewKey)).toEqual([
        'variant:primary',
        'variant:secondary',
        'variant:tertiary',
        'variant:quaternary'
      ])
      expect(result.issues).toEqual([])
    })
  })

  describe('when a set uses numbered variants', () => {
    let component: RawComponent

    beforeEach(async () => {
      // No synthetic set uses numbered variants, so the button is renamed into that shape.
      component = structuredClone(await rawComponent('Demo Controls', 'Synthetic Button'))
      const names = ['Default', 'Variant2', 'Variant3', 'Variant4']
      component.base = {
        ...component.base,
        name: `Property 1=${names[0]}`,
        props: { 'Property 1': names[0]! }
      }
      component.others = component.others.slice(0, 3).map((other, index) => ({
        ...other,
        name: `Property 1=${names[index + 1]}`,
        props: { 'Property 1': names[index + 1]! }
      }))
      component.properties = {
        'Property 1': { type: 'VARIANT', default: names[0]!, options: names }
      }
    })

    it('should read Variant2..4 as hover, selected and selected+hover and say so', () => {
      const result = mapVariants(component, expandVariants(component), defaults)

      expect(
        result.variants.map((variant) => `${variant.flags.selected}/${variant.interaction}`)
      ).toEqual(['false/base', 'false/hover', 'true/base', 'true/hover'])
      expect(result.notes[0]).toContain('numbered-variants')
    })
  })

  describe('when a set encodes a boolean as On/Off', () => {
    it('should expose a boolean prop named after the property', async () => {
      const result = await mapped('Demo Controls', 'HideItem')

      expect(result.props).toEqual([
        {
          name: 'hideItem',
          kind: 'boolean',
          values: ['false', 'true'],
          default: false,
          source: 'HideItem'
        }
      ])
    })

    describe('and a prop name is configured for the property', () => {
      it('should apply the configured prop name', async () => {
        const renamed = await mapped('Demo Controls', 'Switch', {
          propNames: { Switch: 'checked' }
        })

        expect(renamed.props.map((prop) => prop.name)).toEqual(['checked', 'loading'])
      })
    })

    describe('and the property name is a reserved word', () => {
      it('should avoid the reserved word', async () => {
        const reserved = await mapped('Demo Controls', 'Switch')

        expect(reserved.props.map((prop) => prop.name)).toContain('switchValue')
      })
    })
  })

  describe('when option aliases normalise irregular Figma names', () => {
    it('should map the FriendCTA hover and pressed typos onto their base values', async () => {
      const result = await mapped('Demo Controls', 'FriendCTA', {
        stateWords: ['hover', 'pressed', 'loading'],
        optionAliases: {
          AddFriendHover: 'AddHover',
          AddFriendLoading: 'AddLoading',
          RejectHoverPress: 'RejectPressed',
          BlockedPres: 'BlockedPressed'
        }
      })

      expect(result.issues).toEqual([])
      const reject = result.variants.filter((variant) => variant.values.variant === 'reject')
      expect(reject.map((variant) => variant.interaction)).toEqual(['base', 'hover', 'pressed'])
    })
  })

  describe('when two Figma variants collapse into the same view', () => {
    it('should report a variant collision instead of silently dropping one', async () => {
      const result = await mapped('Demo Controls', 'Split Button', {
        optionAliases: { HoverLeft: 'Hover', HoverRight: 'Hover' }
      })

      expect(result.issues.some((issue) => issue.startsWith('variant-collision'))).toBe(true)
    })
  })

  describe('when an option is configured to be skipped', () => {
    it('should drop that variant and leave a note', async () => {
      const result = await mapped('Demo Controls', 'Split Button', {
        skipOptions: ['HoverRight'],
        optionAliases: { HoverLeft: 'Hover' }
      })

      expect(result.variants).toHaveLength(2)
      expect(result.notes[0]).toContain('skipped-variant')
    })
  })
})

describe('when hidden variants were excluded from a capture', () => {
  let component: RawComponent

  beforeEach(() => {
    component = {
      kind: 'set',
      id: 'set',
      name: 'Profile',
      section: [],
      x: 0,
      y: 0,
      hiddenVariants: [{ id: 'hidden', name: 'Style=Hidden' }],
      properties: {
        Style: { type: 'VARIANT', default: 'Hidden', options: ['Hidden', 'Round', 'Square'] }
      },
      base: {
        id: 'round',
        name: 'Style=Round',
        props: { Style: 'Round' },
        tree: { type: 'FRAME', w: 48, h: 48 }
      },
      others: [{ id: 'square', name: 'Style=Square', props: { Style: 'Square' }, patches: [] }]
    }
  })

  it('should offer only observed values and select a visible default', () => {
    const result = mapVariants(component, expandVariants(component), defaults)
    expect(result.props).toEqual([
      {
        name: 'style',
        source: 'Style',
        kind: 'enum',
        values: ['round', 'square'],
        default: 'round'
      }
    ])
  })

  it('should note the visible default it selected', () => {
    const result = mapVariants(component, expandVariants(component), defaults)
    expect(result.notes).toContain('visible-variant-default:Style=Round')
  })

  it('should not mutate the source metadata', () => {
    mapVariants(component, expandVariants(component), defaults)
    expect(component.properties.Style!.default).toBe('Hidden')
    expect(component.properties.Style!.options).toEqual(['Hidden', 'Round', 'Square'])
  })
})

describe('when a non-base enum option is the Figma default', () => {
  let component: RawComponent

  beforeEach(() => {
    const names = ['hover', 'default', 'empty']
    component = {
      kind: 'set',
      id: 'balance',
      name: 'NavCreditsBalance',
      section: [],
      x: 0,
      y: 0,
      properties: { state: { type: 'VARIANT', default: 'empty', options: names } },
      base: {
        id: 'hover',
        name: 'state=hover',
        props: { state: 'hover' },
        tree: { type: 'FRAME', w: 46, h: 24 }
      },
      others: names.slice(1).map((state) => ({
        id: state,
        name: `state=${state}`,
        props: { state },
        patches: [{ at: [], set: { w: state === 'empty' ? 144 : 46 } }]
      }))
    }
  })

  it('should keep the Figma default as the enum default', () => {
    const result = mapVariants(component, expandVariants(component), defaults)
    expect(result.props).toContainEqual({
      name: 'state',
      kind: 'enum',
      values: ['default', 'empty'],
      default: 'empty',
      source: 'state'
    })
  })

  it('should keep default and empty views distinct while grouping hover with default', () => {
    const result = mapVariants(component, expandVariants(component), defaults)
    expect(result.variants.map(viewKey)).toEqual(['state:default', 'state:default', 'state:empty'])
    expect(result.variants.map((variant) => variant.interaction)).toEqual(['hover', 'base', 'base'])
  })
})

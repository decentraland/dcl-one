import { expectTypeOf, it } from 'vitest'
import type { InputExplorerNamesProps } from '../../index'
it('should exclude void-input content from the public Explorer field contract', () => {
  expectTypeOf<InputExplorerNamesProps>().not.toHaveProperty('children')
  expectTypeOf<InputExplorerNamesProps>().not.toHaveProperty('dangerouslySetInnerHTML')
})

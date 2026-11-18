import { expectTypeOf, it } from 'vitest'
import type { InputExplorerNamesProps, InputWebCreatorToolsProps } from '../../index'
it('should exclude void-input content from the public Explorer field contract', () => {
  expectTypeOf<InputExplorerNamesProps>().not.toHaveProperty('children')
  expectTypeOf<InputExplorerNamesProps>().not.toHaveProperty('labelHidden')
  expectTypeOf<InputExplorerNamesProps>().not.toHaveProperty('dangerouslySetInnerHTML')
})
it('should keep the web field free of void-input content and its surface fixed', () => {
  expectTypeOf<InputWebCreatorToolsProps>().not.toHaveProperty('children')
  expectTypeOf<InputWebCreatorToolsProps>().not.toHaveProperty('dangerouslySetInnerHTML')
  expectTypeOf<InputWebCreatorToolsProps>().not.toHaveProperty('surface')
  expectTypeOf<InputWebCreatorToolsProps>().toHaveProperty('labelHidden')
})

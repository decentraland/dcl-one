import { describe, expectTypeOf, it } from 'vitest'
import type { InputExplorerNamesProps, InputWebCreatorToolsProps } from '../../index'

describe('InputField public contracts', () => {
  describe('when the field is the Explorer names input', () => {
    it('should exclude void-input content from its props', () => {
      expectTypeOf<InputExplorerNamesProps>().not.toHaveProperty('children')
      expectTypeOf<Extract<InputExplorerNamesProps, { appearance: 'hud' }>>().not.toHaveProperty('children')
      expectTypeOf<Extract<InputExplorerNamesProps, { appearance: 'hud' }>>().not.toHaveProperty(
        'dangerouslySetInnerHTML'
      )
      expectTypeOf<Exclude<InputExplorerNamesProps, { appearance: 'hud' }>>().not.toHaveProperty('children')
      expectTypeOf<Exclude<InputExplorerNamesProps, { appearance: 'hud' }>>().not.toHaveProperty('labelHidden')
      expectTypeOf<InputExplorerNamesProps>().not.toHaveProperty('labelHidden')
      expectTypeOf<InputExplorerNamesProps>().not.toHaveProperty('dangerouslySetInnerHTML')
    })
  })

  describe('when the field is the web Creator Tools input', () => {
    it('should keep its props free of void-input content and its surface fixed', () => {
      expectTypeOf<InputWebCreatorToolsProps>().not.toHaveProperty('children')
      expectTypeOf<InputWebCreatorToolsProps>().not.toHaveProperty('dangerouslySetInnerHTML')
      expectTypeOf<InputWebCreatorToolsProps>().not.toHaveProperty('surface')
      expectTypeOf<InputWebCreatorToolsProps>().toHaveProperty('labelHidden')
    })
  })
})

import {
  getActionPickerState,
  resolveSelectedAction
} from '../../../packages/@dcl/sdk/src/smart-items/admin-toolkit-ui/utils'

describe('when an admin picks a smart item in the panel', () => {
  it('should keep the action picker closed while no item is picked', () => {
    expect(getActionPickerState(false, 0)).toEqual({
      disabled: true,
      emptyLabel: 'Select action'
    })
  })

  it('should keep the action picker closed, and say why, for an item with no actions', () => {
    expect(getActionPickerState(true, 0)).toEqual({
      disabled: true,
      emptyLabel: 'This item has no actions'
    })
  })

  it('should open the action picker for an item with actions', () => {
    expect(getActionPickerState(true, 3).disabled).toBe(false)
  })
})

describe('when the selected smart item changes', () => {
  const bench = [{ name: 'Sit Here' }, { name: 'Stand' }]
  const confetti = [{ name: 'Explode' }, { name: 'Hide' }]

  it('should use the new item default action when it is one of its actions', () => {
    expect(resolveSelectedAction(confetti, undefined, 'Hide')).toEqual({
      index: 1,
      action: confetti[1]
    })
  })

  it('should fall back to the first action when the default is empty', () => {
    expect(resolveSelectedAction(confetti, undefined, '').action).toBe(confetti[0])
    expect(resolveSelectedAction(confetti, undefined, undefined).index).toBe(0)
  })

  it('should never show or play a stale action from another item', () => {
    // picked name left over from the bench, default also stale
    expect(resolveSelectedAction(confetti, 'Sit Here', 'Sit Here')).toEqual({
      index: 0,
      action: confetti[0]
    })
    expect(resolveSelectedAction(bench, 'Stand', 'Explode').action).toBe(bench[1])
  })

  it('should keep an admin pick that belongs to the item', () => {
    expect(resolveSelectedAction(confetti, 'Hide', 'Explode').index).toBe(1)
  })

  it('should select nothing for an item with no actions', () => {
    expect(resolveSelectedAction([], 'Sit Here', 'Sit Here')).toEqual({
      index: -1,
      action: undefined
    })
  })
})

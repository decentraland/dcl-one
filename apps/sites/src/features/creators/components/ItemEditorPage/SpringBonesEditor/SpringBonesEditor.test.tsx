import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BodyShape } from '@dcl/schemas'
import { TranslationProvider } from '~/intl'
import { getDefaultSpringBoneParams, type BoneNode } from '~/lib/springBones'
import { SpringBonesEditor } from './SpringBonesEditor'

// Armature (not a joint) → Hips → [Tail_springbone_1 → Tail_springbone_2, Hair_springbone]
const bones: BoneNode[] = [
  { name: 'Armature', nodeId: 0, type: 'avatar', isJoint: false, children: [1] },
  { name: 'Hips', nodeId: 1, type: 'avatar', isJoint: true, children: [2, 4] },
  { name: 'Tail_springbone_1', nodeId: 2, type: 'spring', isJoint: true, children: [3] },
  { name: 'Tail_springbone_2', nodeId: 3, type: 'spring', isJoint: true, children: [] },
  { name: 'Hair_springbone', nodeId: 4, type: 'spring', isJoint: true, children: [] }
]
const male = { hash: 'm', bodyShapes: [BodyShape.MALE], bones }
const female = { hash: 'f', bodyShapes: [BodyShape.FEMALE], bones }

describe('SpringBonesEditor', () => {
  it('lists configured chains, tunes their params and adds the remaining roots', async () => {
    const onChange = vi.fn()
    render(
      <SpringBonesEditor
        models={[male]}
        activeHash="m"
        onActiveHashChange={vi.fn()}
        params={{ Tail_springbone_1: getDefaultSpringBoneParams() }}
        onChange={onChange}
      />,
      { wrapper: TranslationProvider }
    )
    expect(screen.getByTestId('spring-bones-counter')).toHaveTextContent('2 / 12')
    fireEvent.change(screen.getByTestId('spring-bones-Tail_springbone_1-stiffness'), { target: { value: '3.5' } })
    expect(onChange).toHaveBeenLastCalledWith({ Tail_springbone_1: expect.objectContaining({ stiffness: 3.5 }) })
    await userEvent.click(screen.getByTestId('spring-bones-add'))
    await userEvent.click(screen.getByTestId('spring-bones-add-option-Hair_springbone'))
    expect(onChange).toHaveBeenLastCalledWith({
      Tail_springbone_1: expect.anything(),
      Hair_springbone: getDefaultSpringBoneParams()
    })
    await userEvent.click(screen.getByTestId('spring-bones-actions-Tail_springbone_1'))
    await userEvent.click(screen.getByTestId('spring-bones-remove-Tail_springbone_1'))
    expect(onChange).toHaveBeenLastCalledWith({})
  })

  it('points a chain at a center bone and clears it again', async () => {
    const onChange = vi.fn()
    const { rerender } = render(
      <SpringBonesEditor
        models={[male]}
        activeHash="m"
        onActiveHashChange={vi.fn()}
        params={{ Tail_springbone_1: getDefaultSpringBoneParams() }}
        onChange={onChange}
      />,
      { wrapper: TranslationProvider }
    )
    await userEvent.click(screen.getByTestId('spring-bones-Tail_springbone_1-center'))
    // The tree shows the skeleton, not the model's other nodes.
    expect(screen.queryByTestId('spring-bones-Tail_springbone_1-center-option-Armature')).toBeNull()
    // Only ordinary bones are pivots; spring bones are listed for context but cannot be picked.
    await userEvent.click(screen.getByTestId('spring-bones-Tail_springbone_1-center-option-Tail_springbone_2'))
    expect(onChange).not.toHaveBeenCalled()
    await userEvent.click(screen.getByTestId('spring-bones-Tail_springbone_1-center-option-Hips'))
    expect(onChange).toHaveBeenLastCalledWith({ Tail_springbone_1: expect.objectContaining({ center: 'Hips' }) })

    rerender(
      <SpringBonesEditor
        models={[male]}
        activeHash="m"
        onActiveHashChange={vi.fn()}
        params={{ Tail_springbone_1: { ...getDefaultSpringBoneParams(), center: 'Hips' } }}
        onChange={onChange}
      />
    )
    await userEvent.click(screen.getByTestId('spring-bones-Tail_springbone_1-center-clear'))
    expect(onChange).toHaveBeenLastCalledWith({ Tail_springbone_1: expect.objectContaining({ center: undefined }) })
  })

  it('copies one chain’s settings onto another', async () => {
    const onChange = vi.fn()
    const tuned = { ...getDefaultSpringBoneParams(), stiffness: 4, center: 'Hips' }
    render(
      <SpringBonesEditor
        models={[male]}
        activeHash="m"
        onActiveHashChange={vi.fn()}
        params={{ Tail_springbone_1: tuned, Hair_springbone: getDefaultSpringBoneParams() }}
        onChange={onChange}
      />,
      { wrapper: TranslationProvider }
    )
    await userEvent.click(screen.getByTestId('spring-bones-actions-Tail_springbone_1'))
    await userEvent.click(screen.getByTestId('spring-bones-copy-Tail_springbone_1'))
    await userEvent.click(screen.getByTestId('spring-bones-actions-Hair_springbone'))
    await userEvent.click(screen.getByTestId('spring-bones-paste-Hair_springbone'))
    expect(onChange).toHaveBeenLastCalledWith({
      Tail_springbone_1: tuned,
      Hair_springbone: expect.objectContaining({ stiffness: 4, center: 'Hips', isRoot: true })
    })
  })

  it('offers a body-shape tab pair for two-model items', async () => {
    const onActiveHashChange = vi.fn()
    render(
      <SpringBonesEditor
        models={[male, female]}
        activeHash="m"
        onActiveHashChange={onActiveHashChange}
        params={{}}
        onChange={vi.fn()}
      />,
      {
        wrapper: TranslationProvider
      }
    )
    await userEvent.click(screen.getByTestId('spring-bones-tab-female'))
    expect(onActiveHashChange).toHaveBeenCalledWith('f')
  })
})

import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { buildBoneTree, type BoneNode } from '~/lib/springBones'
import { BoneTreePicker } from './BoneTreePicker'

const bones: BoneNode[] = [
  { name: 'Armature', nodeId: 0, type: 'avatar', isJoint: false, children: [1] },
  { name: 'Hips', nodeId: 1, type: 'avatar', isJoint: true, children: [2] },
  { name: 'Spine', nodeId: 2, type: 'avatar', isJoint: true, children: [3] },
  { name: 'Hair_springbone', nodeId: 3, type: 'spring', isJoint: true, children: [] }
]

function setup(value?: string) {
  const onChange = vi.fn()
  render(
    <BoneTreePicker
      tree={buildBoneTree(bones)}
      value={value}
      onChange={onChange}
      noneLabel="None"
      clearLabel="Clear"
      disabledType="spring"
    />
  )
  return onChange
}

describe('BoneTreePicker', () => {
  it('picks a bone from the hierarchy and collapses branches', async () => {
    const onChange = setup()
    await userEvent.click(screen.getByTestId('bone-tree'))
    // Descendants start visible; collapsing a branch hides them.
    expect(screen.getByTestId('bone-tree-option-Spine')).toBeInTheDocument()
    await userEvent.click(screen.getByTestId('bone-tree-toggle-Hips'))
    expect(screen.queryByTestId('bone-tree-option-Spine')).toBeNull()
    await userEvent.click(screen.getByTestId('bone-tree-toggle-Hips'))
    await userEvent.click(screen.getByTestId('bone-tree-option-Spine'))
    expect(onChange).toHaveBeenCalledWith('Spine')
  })

  it('clears the selection', async () => {
    const onChange = setup('Spine')
    await userEvent.click(screen.getByTestId('bone-tree-clear'))
    expect(onChange).toHaveBeenCalledWith(undefined)
  })
})

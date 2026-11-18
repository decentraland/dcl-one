import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { HidesEditor } from './HidesEditor'

const contents = { 'male/hat.glb': 'x' }

describe('HidesEditor', () => {
  it('adds body parts and wearable slots to the hides list from two multi-selects', async () => {
    const onChange = vi.fn()
    render(<HidesEditor contents={contents} category="hat" hides={['hair']} onChange={onChange} />, {
      wrapper: TranslationProvider
    })
    expect(screen.getByTestId('hides-editor-categories')).toHaveTextContent('Hair')
    await userEvent.click(screen.getByTestId('hides-editor-body-parts'))
    await userEvent.click(screen.getByTestId('hides-editor-body-parts-option-head'))
    expect(onChange).toHaveBeenCalledWith(['head', 'hair'])
    await userEvent.click(screen.getByTestId('hides-editor-categories'))
    await userEvent.click(screen.getByTestId('hides-editor-categories-option-hair'))
    expect(onChange).toHaveBeenCalledWith([])
  })

  it('offers body_shape only while it is already hidden and nothing for texture wearables', async () => {
    const { rerender } = render(<HidesEditor contents={contents} category="hat" hides={[]} onChange={vi.fn()} />, {
      wrapper: TranslationProvider
    })
    await userEvent.click(screen.getByTestId('hides-editor-categories'))
    expect(screen.queryByTestId('hides-editor-categories-option-body_shape')).not.toBeInTheDocument()
    await userEvent.keyboard('{Escape}')
    rerender(<HidesEditor contents={contents} category="hat" hides={['body_shape']} onChange={vi.fn()} />)
    expect(screen.getByTestId('hides-editor-categories')).toHaveTextContent('Body Shape')
    rerender(<HidesEditor contents={{ 'eyes.png': 'x' }} category="eyes" hides={[]} onChange={vi.fn()} />)
    expect(screen.queryByTestId('hides-editor-body-parts')).not.toBeInTheDocument()
  })

  it('names hides the current category no longer offers', () => {
    render(
      <HidesEditor contents={contents} category="skin" hides={['hair', 'upper_body']} disabled onChange={vi.fn()} />,
      {
        wrapper: TranslationProvider
      }
    )
    expect(screen.getAllByTestId('hides-editor-categories-pill').map(pill => pill.textContent)).toEqual([
      'Hair',
      'Upper Body'
    ])
  })

  it('lists hidden slots as pills when read-only, or says nothing is hidden', () => {
    const { rerender } = render(
      <HidesEditor
        contents={contents}
        category="hat"
        hides={['head', 'hair', 'earring']}
        disabled
        onChange={vi.fn()}
      />,
      { wrapper: TranslationProvider }
    )
    expect(screen.getAllByTestId('hides-editor-categories-pill').map(pill => pill.textContent)).toEqual([
      'Hair',
      'Earring'
    ])
    expect(screen.getAllByTestId('hides-editor-body-parts-pill').map(pill => pill.textContent)).toEqual(['Head'])
    rerender(<HidesEditor contents={contents} category="hat" hides={[]} disabled onChange={vi.fn()} />)
    expect(screen.getByTestId('hides-editor-categories-empty')).toHaveTextContent('Nothing hidden')
    expect(screen.getByTestId('hides-editor-body-parts-empty')).toHaveTextContent('Nothing hidden')
  })
})

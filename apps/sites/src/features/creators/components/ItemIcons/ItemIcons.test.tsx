import { describe, expect, it } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { BodyShapeType } from '~/lib/items'
import { EmotePlayMode } from '~/lib/itemFactory'
import { BodyShapeIcon, CategoryIcon, PlayModeIcon } from './ItemIcons'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

describe('ItemIcons', () => {
  it('renders a category glyph with its label', () => {
    render(<CategoryIcon category="upper_body" withLabel />, { wrapper })
    expect(screen.getByTestId('category-icon')).toHaveTextContent('Upper Body')
    expect(screen.getByRole('img', { name: 'Upper Body' })).toBeInTheDocument()
  })

  it('falls back to the label alone for an unknown category', () => {
    render(<CategoryIcon category="mystery" withLabel />, { wrapper })
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
    expect(screen.getByTestId('category-icon')).toBeInTheDocument()
  })

  it('renders a body-shape glyph with its label', () => {
    render(<BodyShapeIcon bodyShape={BodyShapeType.FEMALE} withLabel />, { wrapper })
    expect(screen.getByTestId('body-shape-icon')).toHaveTextContent('Female')
    expect(screen.getByRole('img', { name: 'Female' })).toBeInTheDocument()
  })

  it('renders a play-mode glyph with its label', () => {
    render(<PlayModeIcon playMode={EmotePlayMode.LOOP} withLabel />, { wrapper })
    expect(screen.getByTestId('play-mode-icon')).toHaveTextContent('Loop')
    expect(screen.getByRole('img', { name: 'Loop' })).toBeInTheDocument()
  })
})

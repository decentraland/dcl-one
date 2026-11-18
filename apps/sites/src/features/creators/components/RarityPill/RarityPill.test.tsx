import { describe, expect, it } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { RarityPill } from './RarityPill'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

describe('RarityPill', () => {
  it('shows the rarity with its max supply by default', () => {
    render(
      <>
        <RarityPill rarity="legendary" testId="legendary" />
        <RarityPill rarity="epic" testId="epic" />
      </>,
      { wrapper }
    )
    expect(screen.getByTestId('legendary')).toHaveTextContent('Legendary (100)')
    expect(screen.getByTestId('legendary')).toHaveAttribute('data-rarity', 'legendary')
    expect(screen.getByTestId('epic')).toHaveTextContent('Epic (1K)')
  })

  it('can drop the supply and tolerates an unknown rarity', () => {
    render(
      <>
        <RarityPill rarity="epic" showSupply={false} testId="epic" />
        <RarityPill rarity="weird" testId="weird" />
      </>,
      { wrapper }
    )
    expect(screen.getByTestId('epic')).toHaveTextContent(/^Epic$/)
    expect(screen.getByTestId('weird')).toHaveTextContent(/^weird$/)
  })
})

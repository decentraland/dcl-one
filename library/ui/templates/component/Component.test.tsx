import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Component } from './Component'

describe('Component', () => {
  describe('when rendered with content', () => {
    it('displays its content', () => {
      render(<Component>Example content</Component>)
      expect(screen.getByText('Example content')).toBeInTheDocument()
    })
  })
})

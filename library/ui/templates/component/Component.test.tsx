import { render, screen } from '@testing-library/react'
import { Component } from './Component'

describe('Component', () => {
  describe('when rendered with content', () => {
    beforeEach(() => {
      render(<Component>Example content</Component>)
    })

    it('should display its content', () => {
      expect(screen.getByText('Example content')).toBeInTheDocument()
    })
  })
})

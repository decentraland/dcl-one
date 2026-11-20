import { fireEvent, render, screen } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { ProfilePic } from './ProfilePic'

describe('ProfilePic', () => {
  describe('when rendered with a picture', () => {
    it('shows the picture with the member name as its text alternative', () => {
      render(<ProfilePic name="Billyjuice" src="https://faces.example/billy.png" />)
      expect(screen.getByRole('img', { name: 'Billyjuice' })).toHaveAttribute(
        'src',
        'https://faces.example/billy.png'
      )
    })

    it('takes another text alternative', () => {
      render(<ProfilePic name="Billyjuice" alt="Billyjuice's profile" src="https://faces.example/b.png" />)
      expect(screen.getByRole('img', { name: "Billyjuice's profile" })).toBeInTheDocument()
    })

    it('is decoration with an empty alternative, beside a visible name', () => {
      render(<ProfilePic name="Billyjuice" alt="" src="https://faces.example/b.png" />)
      expect(screen.queryByRole('img')).toBeNull()
      expect(document.querySelector('img')).toHaveAttribute('alt', '')
    })

    it('renders on the server without a browser', () => {
      const html = renderToString(<ProfilePic name="Billyjuice" src="https://faces.example/b.png" size={40} />)
      expect(html).toContain('alt="Billyjuice"')
      expect(html).toContain('data-size="40"')
    })
  })

  describe('when there is no picture to show', () => {
    it('shows the initial of the name, under the same text alternative', () => {
      render(<ProfilePic name="billyjuice" />)
      const fallback = screen.getByRole('img', { name: 'billyjuice' })
      expect(fallback).toHaveTextContent('B')
      expect(document.querySelector('img')).toBeNull()
    })

    it('shows the initial when the picture fails to load', () => {
      render(<ProfilePic name="Ana" src="https://faces.example/missing.png" />)
      fireEvent.error(screen.getByRole('img', { name: 'Ana' }))
      expect(screen.getByRole('img', { name: 'Ana' })).toHaveTextContent('A')
      expect(document.querySelector('img')).toBeNull()
    })

    it('tries a new picture after the earlier one failed', () => {
      const { rerender } = render(<ProfilePic name="Ana" src="https://faces.example/missing.png" />)
      fireEvent.error(screen.getByRole('img', { name: 'Ana' }))
      rerender(<ProfilePic name="Ana" src="https://faces.example/ana.png" />)
      expect(screen.getByRole('img', { name: 'Ana' })).toHaveAttribute('src', 'https://faces.example/ana.png')
    })

    it('keeps one whole character for a name that starts with an emoji, and a mark for no name', () => {
      const { rerender } = render(<ProfilePic name="🦊fox" />)
      expect(screen.getByRole('img', { name: '🦊fox' })).toHaveTextContent('🦊')
      rerender(<ProfilePic name="" alt="Unknown member" />)
      expect(screen.getByRole('img', { name: 'Unknown member' })).toHaveTextContent('?')
    })

    it('hides the initial from assistive technology with an empty alternative', () => {
      render(<ProfilePic name="Ana" alt="" />)
      expect(screen.queryByRole('img')).toBeNull()
      expect(screen.getByText('A')).toHaveAttribute('aria-hidden', 'true')
    })
  })

  describe('when given the member name colour', () => {
    it('passes it to the styles as a custom property', () => {
      render(<ProfilePic name="Ana" nameColor="rgb(1, 2, 3)" data-testid="pic" />)
      expect(screen.getByTestId('pic').style.getPropertyValue('--profile-pic-name-color')).toBe(
        'rgb(1, 2, 3)'
      )
    })
  })
})

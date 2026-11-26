import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { SearchBar } from './SearchBar'

function Controlled({ onKeyDown }: { onKeyDown?: () => void }) {
  const [value, setValue] = useState('')
  return (
    <SearchBar
      label="Search the blog"
      placeholder="Search..."
      value={value}
      onValueChange={setValue}
      onClear={() => setValue('')}
      onKeyDown={onKeyDown}
    />
  )
}

describe('SearchBar', () => {
  describe('when empty', () => {
    it('shows a named text field with its placeholder and no clear button', () => {
      render(<Controlled />)
      const field = screen.getByRole('textbox', { name: 'Search the blog' })
      expect(field).toHaveAttribute('placeholder', 'Search...')
      expect(field).toHaveAttribute('autocomplete', 'off')
      expect(screen.queryByRole('button', { name: 'Clear search' })).not.toBeInTheDocument()
    })
  })

  describe('when the user types and clears', () => {
    it('reports the value and empties the field from the clear button', async () => {
      const onKeyDown = vi.fn()
      render(<Controlled onKeyDown={onKeyDown} />)
      const field = screen.getByRole('textbox', { name: 'Search the blog' })
      await userEvent.type(field, 'genesis')
      expect(field).toHaveValue('genesis')
      expect(onKeyDown).toHaveBeenCalled()
      await userEvent.click(screen.getByRole('button', { name: 'Clear search' }))
      expect(field).toHaveValue('')
    })
  })

  describe('when no clear handler is given', () => {
    it('never shows the clear button', () => {
      render(<SearchBar label="Search" value="x" onValueChange={() => {}} />)
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })
  })
})

function Hud({ variant, fieldSize }: { variant?: 'dark' | 'light'; fieldSize?: 'md' | 'sm' }) {
  const [value, setValue] = useState('')
  return (
    <SearchBar
      appearance="hud"
      variant={variant}
      fieldSize={fieldSize}
      label="Search friends"
      placeholder="Search friends"
      value={value}
      onValueChange={setValue}
      onClear={() => setValue('')}
      leading={<svg data-testid="magnifier" />}
      clearIcon={<i data-testid="cross" />}
      autoComplete={undefined}
    />
  )
}

describe('when a SearchBar takes the hud appearance', () => {
  it('should wrap the field in a label, with its variant and size and no autocomplete default', () => {
    render(<Hud variant="light" fieldSize="sm" />)
    const field = screen.getByRole('textbox', { name: 'Search friends' })
    const root = field.parentElement!
    expect(root.tagName).toBe('LABEL')
    expect(root).toHaveAttribute('data-variant', 'light')
    expect(root).toHaveAttribute('data-size', 'sm')
    expect(field).not.toHaveAttribute('autocomplete')
  })

  it('should default to the dark variant and the md size', () => {
    render(<Hud />)
    const root = screen.getByRole('textbox', { name: 'Search friends' }).parentElement!
    expect(root).toHaveAttribute('data-variant', 'dark')
    expect(root).toHaveAttribute('data-size', 'md')
  })

  it('should draw the leading visual before the field, hidden from assistive technology', () => {
    render(<Hud />)
    const leading = screen.getByTestId('magnifier').parentElement!
    expect(leading).toHaveAttribute('aria-hidden', 'true')
    expect(leading.nextElementSibling).toBe(screen.getByRole('textbox', { name: 'Search friends' }))
  })

  it("should show the caller's clear icon only while the field has a value, and empty it", async () => {
    render(<Hud />)
    expect(screen.queryByRole('button', { name: 'Clear search' })).not.toBeInTheDocument()
    await userEvent.type(screen.getByRole('textbox', { name: 'Search friends' }), 'moj')
    const clear = screen.getByRole('button', { name: 'Clear search' })
    expect(clear).toContainElement(screen.getByTestId('cross'))
    await userEvent.click(clear)
    expect(screen.getByRole('textbox', { name: 'Search friends' })).toHaveValue('')
    expect(screen.queryByRole('button', { name: 'Clear search' })).not.toBeInTheDocument()
  })
})

describe('when a default SearchBar gets a leading visual', () => {
  it('should keep its box a div and its default clear sign', async () => {
    const Demo = () => {
      const [value, setValue] = useState('a')
      return (
        <SearchBar
          label="Search"
          value={value}
          onValueChange={setValue}
          onClear={() => setValue('')}
          leading={<svg data-testid="magnifier" />}
          variant="light"
          fieldSize="sm"
        />
      )
    }
    render(<Demo />)
    const root = screen.getByRole('textbox', { name: 'Search' }).parentElement!
    expect(root.tagName).toBe('DIV')
    expect(root).not.toHaveAttribute('data-variant')
    expect(root).not.toHaveAttribute('data-size')
    expect(screen.getByRole('button', { name: 'Clear search' })).toHaveTextContent('×')
  })
})

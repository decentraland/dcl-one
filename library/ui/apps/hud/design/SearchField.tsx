// SearchField — the shared SearchBar in its HUD look, with a leading magnifier. Controlled (`value`) or
// uncontrolled (`defaultValue`), with an Enter handler.

import { useState } from 'react'
import { SearchBar } from '@dcl/one-ui'
import { MaskIcon } from './MaskIcon'
import closeIcon from '../assets/backpack/icon-close.webp'

interface SearchFieldProps {
  /** 'light': off-white field with dark ink and a clear button (menu pages). */
  variant?: 'dark' | 'light'
  /** 'sm': a 36px field that fills its container (chat emoji panel). */
  size?: 'md' | 'sm'
  value?: string
  defaultValue?: string
  placeholder?: string
  onChange?: (value: string) => void
  /** Enter pressed, with the field's text. */
  onSubmit?: (value: string) => void
  /** A clear button on the dark variant too (the light one always has it). */
  clearable?: boolean
  /** The clear button emptied the field (after `onChange('')`). */
  onClear?: () => void
}

export function SearchField({
  variant = 'dark',
  size = 'md',
  value,
  defaultValue = '',
  placeholder = 'Search',
  onChange,
  onSubmit,
  clearable = false,
  onClear
}: SearchFieldProps): React.JSX.Element {
  const [internal, setInternal] = useState(defaultValue)
  const isControlled = value !== undefined
  const v = isControlled ? value : internal

  const setValue = (next: string): void => {
    if (!isControlled) setInternal(next)
    onChange?.(next)
  }
  const clears = variant === 'light' || clearable
  return (
    <SearchBar
      appearance="hud"
      variant={variant}
      fieldSize={size}
      label={placeholder}
      placeholder={placeholder}
      autoComplete={undefined}
      value={v}
      onValueChange={setValue}
      onKeyDown={onSubmit != null ? (e) => { if (e.key === 'Enter') onSubmit(e.currentTarget.value) } : undefined}
      leading={
        <svg viewBox="0 0 16 16" width="15" height="15">
          <circle cx="7" cy="7" r="5" fill="none" stroke="currentColor" strokeWidth="1.6" />
          <path d="M11 11l3.5 3.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      }
      clearIcon={<MaskIcon src={closeIcon} size={10} />}
      onClear={
        clears
          ? () => {
              setValue('')
              onClear?.()
            }
          : undefined
      }
    />
  )
}

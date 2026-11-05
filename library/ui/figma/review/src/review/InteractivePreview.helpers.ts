function emailState(value: string, focused: boolean, touched: boolean) {
  if (focused) return value ? 'editing' : 'focus'
  if (!touched || !value) return 'default'
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value) ? 'completed' : 'error'
}

function explorerState(value: string, focused: boolean, password: boolean, reveal: boolean) {
  if (password)
    return reveal
      ? 'passwordFocusedEditingShown'
      : focused
        ? 'passwordFocusedEditingHidden'
        : 'password'
  if (value.length > 15) return 'error'
  if (focused) return value ? 'focusedEditing' : 'focusedEdit'
  return value ? 'edited' : 'empty'
}

// Figma's SearchBar has no separate focus variant: history is the focused, empty bar with recent searches.
function searchState(value: string, focused: boolean, submitted: boolean) {
  if (submitted && value.trim()) return 'notFound'
  if (focused) return value ? 'predictive' : 'history'
  return 'default'
}

// Only these input types support a selection; on others (email, number) setSelectionRange throws
// InvalidStateError, so a remounted email field just keeps focus.
const SELECTABLE = new Set(['text', 'search', 'password', 'tel', 'url'])

function restoreCaret(element: HTMLInputElement, position: number) {
  if (SELECTABLE.has(element.type)) element.setSelectionRange(position, position)
}

export { emailState, explorerState, restoreCaret, searchState }

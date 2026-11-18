import { useEffect, useReducer, useRef } from 'react'

const LETTER_MS = 100
// Held once when the word completes and again on the flip to erase, so a full word shows ~2s, as on sites.
const PAUSE_MS = LETTER_MS * 10

type TypingState = { letterPos: number; wordIndex: number; action: 'write' | 'erase' }

type Tick = { wordLength: number; listLength: number }

function tick(state: TypingState, { wordLength, listLength }: Tick): TypingState {
  if (state.action === 'write') {
    if (state.letterPos >= wordLength - 1) return { ...state, action: 'erase' }
    return { ...state, letterPos: state.letterPos + 1 }
  }
  if (state.letterPos === 0) {
    return { letterPos: 0, wordIndex: listLength ? (state.wordIndex + 1) % listLength : 0, action: 'write' }
  }
  return { ...state, letterPos: state.letterPos - 1 }
}

/** Types out the words of `list` one letter at a time, erasing each before moving to the next, forever. */
export function useTypingListEffect(list: readonly string[]): string {
  const [state, dispatch] = useReducer(tick, { letterPos: 0, wordIndex: 0, action: 'write' })
  const timeout = useRef<ReturnType<typeof setTimeout>>()

  const word = list[state.wordIndex] ?? ''
  const atEnd = state.letterPos >= word.length - 1

  useEffect(() => {
    clearTimeout(timeout.current)
    if (list.length === 0) return
    timeout.current = setTimeout(
      () => dispatch({ wordLength: word.length, listLength: list.length }),
      atEnd ? PAUSE_MS : LETTER_MS
    )
    return () => clearTimeout(timeout.current)
  }, [state, word, list.length, atEnd])

  return word.slice(0, state.letterPos + 1)
}

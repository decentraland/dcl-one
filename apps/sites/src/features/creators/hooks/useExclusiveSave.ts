import { useCallback, useRef, useState } from 'react'

export class SaveInProgressError extends Error {
  constructor() {
    super('Another save is in progress')
    this.name = 'SaveInProgressError'
  }
}

/**
 * Runs one save at a time, from its build until its request settles: item saves are full PUTs built from the stored
 * item, so two in flight would drop each other's change. A save started while another runs is refused.
 */
export function useExclusiveSave() {
  const running = useRef(false)
  const [isSaving, setSaving] = useState(false)
  const run = useCallback(async <T>(task: () => Promise<T>): Promise<T> => {
    if (running.current) throw new SaveInProgressError()
    running.current = true
    setSaving(true)
    try {
      return await task()
    } finally {
      running.current = false
      setSaving(false)
    }
  }, [])
  return { isSaving, run }
}

// The single notifications module (see CONVENTIONS.md): components enqueue toasts here and the
// global <Toasts /> renders them — never ad-hoc toast systems.
import { create } from 'zustand'

export const TOAST_DURATION_MS = 5000

/** `null` renders a plain toast with no status icon. */
export type ToastType = 'success' | 'error' | 'warn' | 'info' | null

export type Toast = {
  id: number
  message: string
  type: ToastType
  durationMs: number
}

export type ToastOptions = {
  type?: ToastType
  durationMs?: number
}

type NotificationsState = {
  toasts: Toast[]
  showToast: (message: string, options?: ToastOptions) => number
  dismissToast: (id: number) => void
}

let nextToastId = 1

export const useNotifications = create<NotificationsState>()(set => ({
  toasts: [],
  showToast: (message, { type = 'success', durationMs = TOAST_DURATION_MS } = {}) => {
    const id = nextToastId++
    set(state => ({ toasts: [...state.toasts, { id, message, type, durationMs }] }))
    return id
  },
  dismissToast: id => set(state => ({ toasts: state.toasts.filter(toast => toast.id !== id) }))
}))

import { reactive, readonly } from 'vue'

export type ToastVariant = 'success' | 'error' | 'info'

export interface ToastOptions {
  title: string
  description?: string
  variant?: ToastVariant
  duration?: number
}

export interface ToastItem extends Required<Omit<ToastOptions, 'description'>> {
  id: number
  description: string
}

const state = reactive<{ items: ToastItem[] }>({ items: [] })
let nextId = 1

const dismiss = (id: number): void => {
  const i = state.items.findIndex((t) => t.id === id)
  if (i >= 0) state.items.splice(i, 1)
}

const push = (opts: ToastOptions): number => {
  const id = nextId++
  const item: ToastItem = {
    id,
    title: opts.title,
    description: opts.description ?? '',
    variant: opts.variant ?? 'info',
    duration: opts.duration ?? 3500,
  }
  state.items.push(item)
  if (item.duration > 0) {
    window.setTimeout(() => dismiss(id), item.duration)
  }
  return id
}

export const toast = {
  success: (title: string, description?: string) =>
    push({ title, description, variant: 'success' }),
  error: (title: string, description?: string) =>
    push({ title, description, variant: 'error', duration: 4500 }),
  info: (title: string, description?: string) =>
    push({ title, description, variant: 'info' }),
  dismiss,
}

export const useToasts = () => readonly(state)

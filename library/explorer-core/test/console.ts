import { expect, onTestFinished, vi } from 'vitest'

export function expectConsoleOutput(method: 'error' | 'warn' | 'debug', calls: unknown[][]): void {
  const output = vi.spyOn(console, method).mockImplementation(() => {})
  onTestFinished(() => {
    try {
      expect(output.mock.calls).toEqual(calls)
    } finally {
      output.mockRestore()
    }
  })
}

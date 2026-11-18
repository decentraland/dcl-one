import { describe, expect, it, vi } from 'vitest'
import { pickFile, pickFiles } from './filePicker'

function lastInput(): HTMLInputElement {
  const inputs = document.querySelectorAll('input[type="file"]')
  return inputs[inputs.length - 1] as HTMLInputElement
}

function withFiles(input: HTMLInputElement, files: File[]) {
  const list = {
    ...files,
    length: files.length,
    item: (index: number) => files[index] ?? null
  } as unknown as FileList
  Object.defineProperty(input, 'files', { value: list })
}

const file = (name: string) => new File(['x'], name)

describe('pickFiles', () => {
  it('opens a dialog with the requested filter and resolves with what was picked', async () => {
    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => undefined)
    const pending = pickFiles({ accept: '.glb,.zip', multiple: true })
    const input = lastInput()
    expect(click).toHaveBeenCalled()
    expect(input.accept).toBe('.glb,.zip')
    expect(input.multiple).toBe(true)

    withFiles(input, [file('a.glb'), file('b.glb')])
    input.dispatchEvent(new Event('change'))

    await expect(pending).resolves.toEqual([expect.objectContaining({ name: 'a.glb' }), expect.anything()])
    // The input never outlives the dialog.
    expect(input.isConnected).toBe(false)
    click.mockRestore()
  })

  it('resolves empty when the dialog is dismissed', async () => {
    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => undefined)
    const pending = pickFile({ accept: 'image/png' })
    const input = lastInput()
    input.dispatchEvent(new Event('cancel'))

    await expect(pending).resolves.toBeNull()
    expect(input.isConnected).toBe(false)
    click.mockRestore()
  })
})

// Host-facing side effects (see CONVENTIONS.md): the one place the app asks the user for files. An
// Electron host swaps this for `dialog.showOpenDialog` over RPC without touching any component.

export type FilePickOptions = {
  /** Same syntax as the `accept` attribute: `.glb,.zip`, `image/png`, … */
  accept?: string
  multiple?: boolean
}

/**
 * Opens the OS file dialog and resolves with what was picked, or an empty list when it is dismissed.
 * The input lives outside React: it is created per call and removed once the dialog closes.
 */
export function pickFiles({ accept, multiple = false }: FilePickOptions = {}): Promise<File[]> {
  return new Promise(resolve => {
    const input = document.createElement('input')
    input.type = 'file'
    input.multiple = multiple
    if (accept) input.accept = accept
    input.style.display = 'none'
    document.body.appendChild(input)

    const settle = (files: File[]) => {
      input.remove()
      resolve(files)
    }
    input.addEventListener('change', () => settle(input.files ? Array.from(input.files) : []), { once: true })
    // Dismissing the dialog fires `cancel`, so the promise settles either way and nothing leaks.
    input.addEventListener('cancel', () => settle([]), { once: true })
    input.click()
  })
}

/** `pickFiles` for a single file: null when the dialog is dismissed. */
export async function pickFile(options: Omit<FilePickOptions, 'multiple'> = {}): Promise<File | null> {
  const [file] = await pickFiles(options)
  return file ?? null
}

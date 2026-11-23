// What a run leaves behind, under `report/<spec file>/<scenario>/`: every console command and
// its reply (`commands.jsonl`, with credentials redacted by the Explorer), the final frame
// (`final.png`), for a visual scenario the frame, the reference and the diff, and any file the
// scenario attaches (`attach`).
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import type { CommandRecord } from './explorer.ts'
import { REPORT_DIR } from './paths.ts'

export function safeName(name: string): string {
  return name.replace(/[^A-Za-z0-9._-]+/g, '_').replace(/^_+|_+$/g, '').slice(0, 120) || 'unnamed'
}

export class ScenarioReport {
  readonly dir: string
  readonly #commands: CommandRecord[] = []
  readonly #attachments = new Map<string, () => Uint8Array | string>()

  constructor(file: string, scenario: string, root: string = REPORT_DIR) {
    this.dir = path.join(root, safeName(file), safeName(scenario))
  }

  record(command: CommandRecord): void {
    this.#commands.push(command)
  }

  /** A file the report writes beside the others, read when the report is written, so it holds
   *  what happened up to the scenario's end whatever the outcome. */
  attach(name: string, content: () => Uint8Array | string): void {
    this.#attachments.set(name, content)
  }

  get commandCount(): number {
    return this.#commands.length
  }

  async write(files: Record<string, Uint8Array | string> = {}): Promise<void> {
    await mkdir(this.dir, { recursive: true })
    const lines = this.#commands.map((command) => JSON.stringify(command)).join('\n')
    await writeFile(path.join(this.dir, 'commands.jsonl'), lines === '' ? '' : `${lines}\n`)
    for (const [name, content] of Object.entries(files)) await writeFile(path.join(this.dir, name), content)
    for (const [name, content] of this.#attachments) await writeFile(path.join(this.dir, name), content())
  }
}

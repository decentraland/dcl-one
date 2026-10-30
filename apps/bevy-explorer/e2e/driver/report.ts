// What a run leaves behind, under `report/<spec file>/<scenario>/`: every console command and
// its reply (`commands.jsonl`, with credentials redacted by the Explorer), the final frame
// (`final.png`) and, for a visual scenario, the frame, the reference and the diff.
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

  constructor(file: string, scenario: string, root: string = REPORT_DIR) {
    this.dir = path.join(root, safeName(file), safeName(scenario))
  }

  record(command: CommandRecord): void {
    this.#commands.push(command)
  }

  get commandCount(): number {
    return this.#commands.length
  }

  async write(files: Record<string, Uint8Array | string> = {}): Promise<void> {
    await mkdir(this.dir, { recursive: true })
    const lines = this.#commands.map((command) => JSON.stringify(command)).join('\n')
    await writeFile(path.join(this.dir, 'commands.jsonl'), lines === '' ? '' : `${lines}\n`)
    for (const [name, content] of Object.entries(files)) await writeFile(path.join(this.dir, name), content)
  }
}

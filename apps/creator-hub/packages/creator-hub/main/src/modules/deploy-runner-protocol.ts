/**
 * The lines the deploy runner (src/deploy-runner.ts) prints on stdout for cli.ts to read: one
 * when the scene's deploy server is listening, or one with the error that stopped it. The error
 * keeps the `CliError` name (`BUNDLE_REBUILD_FAILED`, `DEPLOY_MISSING_TARGET_CONTENT`…), which
 * the publish dialog matches on, so it is sent as JSON rather than as the CLI's plain message.
 */
const READY = '[deploy-runner] ready';
const FAILED = '[deploy-runner] failed ';

export type DeployRunnerFailure = { name: string; message: string };

export type DeployRunnerEvent = { kind: 'ready' } | { kind: 'failed'; error: DeployRunnerFailure };

export function formatDeployRunnerReady(): string {
  return `${READY}\n`;
}

export function formatDeployRunnerFailure(error: unknown): string {
  const failure: DeployRunnerFailure =
    error instanceof Error
      ? { name: error.name || 'Error', message: error.message }
      : { name: 'Error', message: String(error) };
  return `${FAILED}${JSON.stringify(failure)}\n`;
}

/** Reads one complete stdout line of the runner. Any other line (the CLI's own output) is null. */
export function parseDeployRunnerLine(line: string): DeployRunnerEvent | null {
  const text = line.trim();
  if (text === READY) return { kind: 'ready' };
  if (!text.startsWith(FAILED)) return null;
  try {
    const parsed = JSON.parse(text.slice(FAILED.length)) as Partial<DeployRunnerFailure>;
    return {
      kind: 'failed',
      error: {
        name: typeof parsed.name === 'string' && parsed.name ? parsed.name : 'Error',
        message: typeof parsed.message === 'string' ? parsed.message : '',
      },
    };
  } catch {
    return { kind: 'failed', error: { name: 'Error', message: text.slice(FAILED.length) } };
  }
}

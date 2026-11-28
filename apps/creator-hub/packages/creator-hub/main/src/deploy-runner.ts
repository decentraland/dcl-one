import path from 'node:path';
import { pathToFileURL } from 'node:url';

import {
  formatDeployRunnerFailure,
  formatDeployRunnerReady,
} from './modules/deploy-runner-protocol';

/**
 * Runs `sdk-commands deploy --programmatic` for the scene in the working directory, in a process
 * of its own (cli.ts starts one per publish and kills it when the next publish starts).
 *
 * Programmatic mode returns as soon as the deploy server listens and leaves that server running,
 * which is what the publish dialog talks to. The CLI's own entry point exits right after the
 * command returns, so it cannot be used here: this runner calls `runSdkCommand` itself, prints
 * a ready line, and stays alive on the server until it is killed.
 *
 * The scene's `sdk-commands` used to be imported into the Creator Hub main process instead. Node
 * keeps every module and every resolved path for the life of the process, so after the scene's
 * dependencies changed on disk (an install that moved `@dcl/ecs` out of `@dcl/sdk`, an SDK
 * update), each later publish bundled against paths that no longer existed, until the app was
 * restarted. A fresh process per publish reads `node_modules` as it is now.
 */
async function main(): Promise<void> {
  const dist = path.join(process.cwd(), 'node_modules', '@dcl', 'sdk-commands', 'dist');
  const load = (file: string) => import(pathToFileURL(path.join(dist, file)).href);
  const { initComponents } = await load('components/index.js');
  const { runSdkCommand } = await load('run-command.js');
  const components = await initComponents();
  await runSdkCommand(components, 'deploy', process.argv.slice(2));
  process.stdout.write(formatDeployRunnerReady());
}

main().catch(error => {
  // Exit once the line is flushed: a pipe write is asynchronous on macOS.
  process.stdout.write(formatDeployRunnerFailure(error), () => process.exit(1));
});

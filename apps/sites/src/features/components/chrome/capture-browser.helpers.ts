import { availableParallelism, freemem, loadavg } from "node:os";
import {
  findBrowser,
  openSession,
} from "../../../../../../library/ui/figma/review/scripts/capture-browser.mjs";

export type CaptureSession = Awaited<ReturnType<typeof openSession>>;

// The browser suites run beside every other Sites test file. On a loaded CI runner some launches of
// Chrome stayed alive without writing their DevTools port for the whole 30-second limit, at times
// after every other test file had finished. A launch that is not ready within openSession's start
// limit is stopped and replaced, up to START_ATTEMPTS launches in all; only the browser start is
// repeated, never an assertion. Any other startup failure, such as a browser that exits, fails at
// once.
const START_ATTEMPTS = 3;
const START_LIMIT_MS = 30_000;
// Per launch: the port file, up to 10 seconds each for the DevTools endpoint and socket, and up to
// 7 seconds of teardown.
export const START_BUDGET_MS = START_ATTEMPTS * (START_LIMIT_MS + 27_000);

/** Starts the capture browser for a suite, replacing a launch that never becomes ready. */
export async function startCaptureBrowser(
  suite: string,
  signal: AbortSignal,
): Promise<CaptureSession> {
  const candidates = [
    [process.env.BROWSER_PATH, "BROWSER_PATH"],
    [process.env.CHROME_PATH, "CHROME_PATH"],
    [
      "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
      "macos-chrome",
    ],
    ["/usr/bin/google-chrome", "linux-chrome"],
    ["/usr/bin/chromium", "linux-chromium"],
  ] as const;
  const executable = await findBrowser(candidates.map(([file]) => file));
  const source = candidates.find(([file]) => file === executable)![1];
  try {
    const address = process.env.DBUS_SESSION_BUS_ADDRESS;
    const type = address?.split(":", 1)[0];
    console.info(
      `${suite} launch context:`,
      JSON.stringify({
        source,
        cpus: availableParallelism(),
        load: loadavg().map((value) => Number(value.toFixed(2))),
        freeMemoryMiB: Math.floor(freemem() / 1024 / 1024),
        dbus: !address
          ? "unset"
          : ["unix", "tcp", "autolaunch"].includes(type!)
            ? type
            : "other",
      }),
    );
  } catch {
    // Diagnostics must not prevent the browser launch.
  }
  for (let attempt = 1; ; attempt++) {
    try {
      return await openSession(executable, {
        signal,
        startTimeoutMs: START_LIMIT_MS,
      });
    } catch (error) {
      if (
        signal.aborted ||
        attempt === START_ATTEMPTS ||
        (error as Error).message !== "Capture browser did not start"
      )
        throw error;
      // openSession has already stopped this browser and printed its startup diagnostics.
      console.warn(
        `Capture browser launch ${attempt} of ${START_ATTEMPTS} was not ready; launching another`,
      );
    }
  }
}

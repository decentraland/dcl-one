# Session recorder

The session recorder lets you press **Record** in the AI assistant chat, talk while you work in the editor, and send what you said and did to the assistant as one message. This page describes the contracts between its parts: the event format, where untrusted text is cleaned, what lands on disk, how long it stays, and how transcription runs. Read it before you change any of them.

## How a recording flows

A recording crosses four processes. Each part has one owner:

1. The inspector iframe records what you do. Its trackers live in `packages/inspector/src/lib/recorder/` (DOM, ECS, Redux, renderer) and `src/lib/renderer/bevy/scene-observer.ts` (what a running scene shows). They only listen while the host has switched capture on (`set_recording`).
2. The inspector sends batches over the scene RPC (`record_events`). The renderer's handler (`renderer/src/modules/rpc/scene/server.ts`) keeps plain objects only and forwards them to the active session.
3. The renderer's recording controller (`renderer/src/modules/recording/controller.ts`) owns the recorder state. It opens the microphone, streams audio and events to main over unlogged fire-and-forget channels (`recording.audio`, `recording.events`), and dispatches every state change to the `ai.recorder` slice.
4. Main (`main/src/modules/recording/`) writes the session folder, transcribes it on stop, builds the timeline, and returns the result the chat attaches to your next message.

## Event contract

Every event is a `RecordedEvent`: a wall-clock time `t`, a `kind`, one line of `text` that reads on its own, and optional `entity`, `area`, pointer coordinates, and `label`.

- The kinds are listed in `RECORDED_EVENT_KINDS`. The list exists twice, in `packages/inspector/src/lib/recorder/types.ts` and `packages/creator-hub/shared/types/recording.ts`, because the host never imports inspector code. When you add a kind, add it to both lists. Main drops events whose kind it doesn't know.
- `label` is set on `scene-appear` and `scene-remove`. It names what appeared without the event's own wording (for example, `On-screen text "BOSS!"`). The timeline uses it to name candidates in a `SAID` line's note and falls back to `text` when an older inspector sends no label.

## Untrusted text

Scene content is untrusted. Entity names, glTF mesh names, hover text, and on-screen text come from the scene and its assets, and the assistant that reads the timeline runs with full permissions in your project. The recorder keeps that text from passing for your speech or for instructions:

- The inspector quotes every scene- or UI-derived name with `JSON.stringify` where it builds the event text (`quote` and `entityLabel` in `recorder/text.ts`), so a quote inside a name can't end the quotation.
- Main is the trust boundary. `sanitize.ts` checks each event's shape, drops unknown kinds, strips control characters, collapses whitespace in every string field, and caps lengths (200 characters of text, 120 of entity name, 60 of area). A session keeps at most 20,000 events. The project title is cleaned the same way before it goes into `SESSION.md`.
- The timeline renders names and on-screen text in a `SAID` line's note with `JSON.stringify`.
- The system prompt (`main/src/modules/ai-prompt.ts`) tells the assistant that only `SAID` lines are your words and that entity, mesh, and scene text is data, not instructions.

Secrets never enter the event stream. The DOM tracker treats password inputs, `[data-sensitive]` elements, masked text areas, and credential-shaped labels as secret: it records that you edited them, never their value or the characters you typed. Click labels never read text inside a `<textarea>` or a `[data-sensitive]` element. The ECS tracker hides credential fields at any depth of a component value (`Rewards.dispenserKey`, `VideoControlState.streamKey`, and any field whose name contains password, secret, token, API key, private key, or mnemonic).

## Session folder

Each recording gets its own folder under the app's user data folder: `recordings/<project>-<hash>/<date>_<time>.<ms>-<hex>/`. It holds:

| File              | Contents                                                                     |
| ----------------- | ---------------------------------------------------------------------------- |
| `audio.wav`       | 16 kHz mono 16-bit PCM, written header first. Deleted when the session ends. |
| `events.jsonl`    | One sanitized `RecordedEvent` per line.                                      |
| `transcript.json` | The transcribed words with their timings.                                    |
| `SESSION.md`      | The full timeline. The context sent to the assistant links it.               |

A recording stops by itself after 10 minutes (`MAX_RECORDING_MS`), and main drops audio past that length.

## Retention

The recorder keeps as little as the assistant needs:

- Main deletes `audio.wav` when the session ends, whatever the outcome: transcribed, skipped, failed, or ended early because the project or the app closed. Nothing reads the audio after the transcription. If the delete fails, main logs it and the 30-day prune removes the file later.
- Discarding a recording deletes its folder right away. Main refuses to delete a session that is still recording or being transcribed.
- Each project keeps its 20 newest sessions.
- At startup, main deletes every session folder that hasn't changed for `RECORDING_RETENTION_DAYS` (30) days, and project folders left empty. Age is the folder's modification time.

The consent dialog states these rules. It reads the limits from the shared constants, so the dialog and the code can't disagree.

## Transcription

Speech is transcribed on your computer. The whisper bindings (`@fugood/whisper.node`) and the models are not bundled with the app:

- The bindings install on first use into the `recording-tools` folder with `npm ci --ignore-scripts` from a lockfile for each platform. An install only counts as done when npm has written `node_modules/.package-lock.json`, which it writes last.
- The models download from a pinned commit and are checked against their SHA-256 before use. A download fails when no data arrives for 60 seconds.
- A worker process (`main/src/transcribe-worker.ts`) runs whisper. It reads its job from a file (`transcribe-job.json`) and writes its result to another file (`transcribe-result.json`). Nothing it transcribes goes to stdout, because the app log records the stdout of every child process and **Report Issue** can upload that log.
- The worker gets 10 minutes. On a timeout, main kills the worker and waits for it to exit before reporting the error.

While main waits for the tools or the transcription, **Send without transcript** calls `recording.skipTranscription`. Main kills a running worker, lets a shared install continue in the background, and resolves the stop with the full action timeline, an empty transcript, and `transcriptionSkipped`.

Transcription and background install failures go to Sentry with the tag `source: recording`.

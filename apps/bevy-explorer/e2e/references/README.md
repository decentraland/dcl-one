Each `<id>.png` is the reference frame of the scene `<id>` in `specs/visual/scenes.ts`: the engine's canvas as `/screenshot` returns it.
The suite requests a 960x540 outer browser window; the canvas size depends on the runner's browser chrome and display scaling (944x445 on the Windows runner used for these references).
Compare the captured canvas at its original dimensions; do not resize it to the requested outer window size.
They are recorded only in CI, on the suite's Windows GPU runner with `DCL_E2E_RECORD=1`, and committed through Git LFS.
A reference counts as approved once a reviewer has checked it against the scene's source in `scenes/<id>`.
A change of the runner image re-records them.

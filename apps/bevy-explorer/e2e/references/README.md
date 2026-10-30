Each `<id>.png` is the reference frame of the scene `<id>` in `specs/visual/scenes.ts`: 960x540, the engine's canvas as `/screenshot` returns it.
They are recorded only in CI, on the suite's Windows GPU runner with `DCL_E2E_RECORD=1`, and committed through Git LFS.
A reference counts as approved once a reviewer has checked it against the scene's source in `scenes/<id>`.
A change of the runner image re-records them.

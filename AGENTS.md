# dcl-one

## Explorer automation

The end-to-end suite in `apps/bevy-explorer/e2e` and the Explorer's console surface it drives
(`rendering/bevy-explorer/source`: `src/automation.rs`, `src/world_state.rs`,
`react-web/src/lib/hudAutomation.ts`, `local-harness/web-driver`, documented in the readme's
"Agents and tests") read the code they test, so a change to that code updates them in the same
change. That is any of: a HUD control's role, accessible name, placeholder, `aria-*` state or
visibility; a page's heading or tablist; a key binding; a console command's name, arguments or
reply; a launch option or the trusted-input contract; a visual scene, its parcels or the capture
window; a move of a path under `apps/bevy-explorer`, `tooling/bevy-e2e-*.py` or the workflow
`bevy-explorer-e2e.yml`; a rename of the CI workflow, its jobs or the web artifact.
`apps/bevy-explorer/e2e/README.md`, "Keeping the suite true", maps each change to what it touches
and the checks to run.

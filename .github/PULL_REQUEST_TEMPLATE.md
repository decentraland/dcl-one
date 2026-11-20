# Pull Request Description

<!-- Follow docs/contributing.md for the PR title and branch name. -->

<!-- Short: problem and behavior first. Tables for before/after or test
results. -->

## Evidence of the original problem (optional)

<!-- Include or link to evidence that shows what was wrong or missing before
this change. For example, share a test that fails on the base branch, a
screenshot of the old behavior, or a baseline measurement. Explain what
you expected, what the evidence shows, and why the difference mattered. -->

## What does this PR change?

<!-- What and why; issue (#123); measured before/after; test scene for SDK
features. -->

## Test Instructions

<!-- Copy-pasteable commands and expected results; separate checks run from
not run. -->

### Prerequisites

### Test Steps

1. Run `<command or action>`.
2. Verify `<expected result>`.

**Automation** (if applicable):

### Additional Testing Notes

## Quality Checklist

- [ ] Relevant checks were run; results and any checks not run are listed above.
- [ ] Documentation has been updated, if required.
- [ ] Performance impact has been considered.
- [ ] For changes covered by `docs/design-feasibility.md`, the design verdict, platform coverage and unresolved blockers are stated.
- [ ] For SDK features: a test scene is included or linked (or not applicable).

## Code Review Reference

<!-- Follow repository-root AGENTS.md and services/catalyst/docs/comms/README.md.
Apply docs/design-feasibility.md before code-quality review when its trigger applies;
carry any unresolved design blocker into the verdict even if implementation checks pass.
Apply rendering/bevy-explorer/docs/guidelines/contributing.md#public-prose:
preserve named client facts. Porting PRs follow the source-attribution workflow in
rendering/bevy-explorer/docs/guidelines/porting-from-unity.md; keep other implementation
provenance in private reports.
Verify acceptance claims against current readiness evidence. Keep private links,
findings and credentials out of the public PR. -->

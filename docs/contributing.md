# Contributing to dcl-one

## Branch names and pull request titles

Name new work branches `<type>/<slug>`. Use a short slug with lowercase letters,
digits, and single hyphens between words. Include the component when it helps
identify the work, as in `fix/explorer-volume-slider`.

Write PR titles as `<type>: <summary>` or `<type>(<scope>): <summary>`. Prefer a
component scope when the change has a clear home in the repository. Start the
summary with a lowercase verb and keep the spelling of names and acronyms in
the rest of the summary.

Use the same change types for both:

| Type | Use |
| --- | --- |
| `feat` | Add behavior or a capability |
| `fix` | Correct existing behavior |
| `perf` | Improve performance |
| `refactor` | Restructure code while preserving behavior |
| `test` | Add or change tests |
| `docs` | Change documentation |
| `style` | Change code formatting |
| `build` | Change build tooling, packaging, or build dependencies |
| `ci` | Change continuous integration |
| `chore` | Do other maintenance |
| `revert` | Revert a prior change |

Match the branch and title type when opening the PR. If the work changes,
update the title to describe the resulting change and keep the existing PR head.
For a breaking change, add `!` before the title's colon, as in
`feat(protocol)!: replace the room handshake`. The validator also accepts legacy
`break:` titles during migration.

Use the repository's established names for integration, release, hotfix, and
bot-managed branches, such as `main`, `release/<id>`, `hotfix/<slug>`,
`dependabot/**`, and `update/nixpkgs`. Release PRs may use `release: <summary>`.

The **Validate PR title** check reports title format issues on PRs. It is
advisory. Branch names are contributor guidance.

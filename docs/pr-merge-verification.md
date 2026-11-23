# PR merge verification

1. **Read the reviews.** Read review bodies and inline comments, including approving
   reviews. A concrete report that the PR fails when combined with the target branch
   blocks merging. Reproduce it on the merge candidate and record a passing check for
   the fix or evidence that the reported failure does not apply.
2. **Verify what was tested.** Fetch the current PR head and target tip from GitHub and
   record their SHAs. Update a behind branch from its target and let PR CI run on the
   updated head. Use `git merge-base --is-ancestor <base-sha> <head-sha>` to check that
   the head includes the target tip. If using an existing run for a behind branch, verify
   from its checkout and the merge commit's parents that it tested the current head and
   target together.
3. **Verify the jobs and steps.** Use the workflow definitions for that candidate and
   the affected paths to identify relevant builds, tests and guards, including checks
   added on the target branch. Open their runs and confirm the relevant steps ran and
   passed. A green summary can omit skipped jobs and checks added after the run.
   Run missing checks against the candidate before merging.
4. **Recheck before merging.** Fetch the two SHAs again immediately before merging.
   Repeat verification if either changed. With GitHub CLI, pass
   `--match-head-commit <head-sha>` to pin the verified head. Enable auto-merge only
   when the merge process will revalidate the same checks if the target advances.
   An authorized approval override leaves the verification steps in force.
5. **Verify the result.** Record the merge SHA and inspect its relevant post-merge CI.
   Report any failure with its run or job link and compare it with the preceding target
   revision to identify regressions introduced by the merge.

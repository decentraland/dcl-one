# PR merge verification

1. **Read the reviews.** Read review bodies and inline comments, including approving
   reviews. A concrete report that the PR fails when combined with the target branch
   blocks merging. Reproduce it on the merge candidate and record a passing check for
   the fix or evidence that the reported failure does not apply.
2. **Verify what was tested.** Read the PR head and target branch name from GitHub. Get
   the live target tip with `git ls-remote origin refs/heads/<target-branch>` and fetch
   both commits. Verify the commit checked out by CI using one of these paths.

   - CI checked out the current PR head, and
     `git merge-base --is-ancestor <target-tip-sha> <head-sha>` exits 0.
   - CI checked out GitHub's PR test merge commit. Its first parent is the live target
     tip and its second parent is the current PR head. Use
     `git show -s --format=%P <tested-merge-sha>` to read the parents in order.

   If neither path matches, update the PR branch from its target and wait for fresh CI.
3. **Verify the jobs and steps.** Use the workflow definitions for that candidate and
   the affected paths to identify relevant builds, tests and guards, including checks
   added on the target branch. Open their runs and confirm the relevant steps ran and
   passed. A green summary can omit skipped jobs and checks added after the run.
   Run missing checks against the candidate before merging.
4. **Recheck before merging.** Fetch the two SHAs again immediately before merging.
   Repeat verification if either changed. With GitHub CLI, pass
   `--match-head-commit <head-sha>` to pin the verified head. For auto-merge, require
   fresh results for the checks in step 3 when the target advances. Use branch
   protection that requires an up-to-date branch and those checks, or a merge queue
   that runs and requires those checks on `merge_group`.
   An authorized approval override leaves the verification steps in force.
5. **Verify the result.** Record the merge SHA and inspect its relevant post-merge CI.
   Report any failure with its run or job link and compare it with the preceding target
   revision to identify regressions introduced by the merge.

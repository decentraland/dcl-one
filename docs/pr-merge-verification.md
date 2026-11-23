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
   - CI checked out GitHub's PR test merge commit. Take its SHA from the job's checkout
     log, fetch it with `git fetch origin <tested-merge-sha>`, and read its parents with
     `git show -s --format=%P <tested-merge-sha>`. The first parent must be the live
     target tip and the second must be the current PR head.

   If neither path can be verified, update the PR branch from its target and wait for
   fresh CI.
3. **Verify the jobs and steps.** Use the workflow definitions for that candidate and
   the affected paths to identify relevant builds, tests and guards, including checks
   added on the target branch. Open their runs and confirm the relevant steps ran and
   passed. A green summary can omit skipped jobs and checks added after the run.
   Run missing checks against the candidate before merging.
4. **Recheck before merging.** Fetch the two SHAs again immediately before merging.
   Repeat verification if either changed. For a direct merge, pass
   `--match-head-commit <head-sha>` with GitHub CLI and merge immediately after the
   recheck. This pins the PR head. The target can still advance during that interval.
   Enable auto-merge only when the target already requires an up-to-date branch and
   every check in step 3, or uses a merge queue that runs and requires those checks on
   `merge_group`. Otherwise merge manually after verification. An authorized approval
   override leaves these verification steps in force.
5. **Verify the result.** Record the merge SHA and inspect its relevant post-merge CI.
   For a direct squash merge, compare its first parent with the verified target tip to
   detect a target advance during the merge. Report any failure with its run or job
   link and compare it with the preceding target revision to identify regressions
   introduced by the merge.

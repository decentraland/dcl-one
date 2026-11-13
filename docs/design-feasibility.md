# Design feasibility gate

Apply before implementation planning for a runtime feature, resource policy or
architecture change whose outcome depends on platform capabilities, external
services or ownership across subsystems. Reapply when those assumptions change.
Mechanical edits with no behavior change do not need this gate.

The gate answers **whether the design can deliver the requested outcome**. Code
correctness, an accepted design label and passing CI are separate evidence.

## 1. Recover the outcome

Read the originating requirement and current product/platform contract before the
proposed implementation. State the user-visible failure to prevent, the supported
targets and the smallest useful outcome this slice must deliver. Identify later
work separately. A PR's description of its own mechanism is not an independent
acceptance criterion.

For each exclusion, record the user-visible consequence and its approval source.
An implementation preference or acknowledgement that an API is missing does not
by itself accept leaving the affected product outcome unsupported. Reuse an
existing explicit scope decision; ask only about a material unresolved choice.

## 2. Build the coverage table

Use one row per supported execution environment and independently constrained
resource or failure domain. Follow real data and ownership paths, including
dependencies and work outside the main process. For each row record:

| Required field | Evidence to supply |
| --- | --- |
| Outcome and workload | What must improve; sustained and burst workloads that can break it |
| Lifetime and cost | Where the resource is created, copied, transferred, retained and released; live owners versus evictable retention |
| Production signal | Exact API/counter, units, numerator, denominator, scope, update delay and excluded allocations/work |
| Availability | Supported production configuration; behavior when absent, stale, denied or disabled |
| Control and fallback | What changes the threatened resource, who owns that action, and what bounds growth without the signal |
| Acceptance evidence | A falsifiable scenario, expected outcome, environment and evidence source or planned check |

Trace the chain **workload → resource growth → observation → decision → action →
actual relief**. Verify each link in source and primary platform documentation;
date external capability claims. Label measurements, estimates, configured
budgets and assumptions distinctly. A resource limit or address-space ceiling
is not a measurement of available capacity. A rate limit is not a residency cap.

Treat an unavailable observation as **unknown**. Use **not applicable** only when
the resource or failure mode is absent in that environment. A present resource
with no telemetry still needs a justified policy. An indirect metric needs a
demonstrated relationship to the threatened resource; correlation is an
assumption until established. Shared physical hardware does not establish that
relationship between application counters.

Direct measurement is not mandatory. A conservative estimate or bounded policy
can support the outcome if the design identifies its coverage, uncertainty,
chosen budget, and behavior when the estimate is wrong. Explain how ownership,
in-flight work and deferred destruction affect the time to relief. Include the
case where nothing eligible can be released without invalidating active users.

## 3. Try to falsify the design

Construct a supported workload where the threatened resource grows while the
chosen signal stays healthy, disappears or lags behind. Follow the production
path without diagnostic overrides. Check the largest plausible item, concurrent
work, long sessions and active-owner retention where relevant. Determine whether
the action can take effect before the failure it is meant to prevent.

Separate evidence for detecting the condition, performing the action and
improving the outcome. A simulated trigger proves only the path after injection;
a counter of actions proves only those actions. Compilation and another
platform's successful run establish neither production detection nor benefit on
the target platform. Define an observable resource bound or other outcome proxy
when the underlying resource cannot be measured directly, and state its limits.

Before implementation, settle feasibility-critical unknowns from source, platform
contracts or a focused investigation. Record a platform-specific validation plan
for the later implementation. Runtime evidence is not expected from unwritten
code; an unsupported central assumption cannot be deferred to code review.

## 4. Issue the decision first

Return these fields before implementation details:

1. **READY** or **BLOCKED**, with the outcome and platforms covered.
2. **Decisive evidence:** source locations and platform contracts for the causal
   chain, plus the coverage table.
3. **Counterexample:** concrete workload, observation, resulting behavior and
   user-visible consequence; distinguish demonstrated facts from hypotheses.
4. **Conditions to proceed:** design change or investigation needed, its
   acceptance check, and any unresolved user scope decision.

**BLOCKED:** a material required failure mode has neither a usable observation
and effective action nor a justified independent bound/fallback; or a central
assumption remains unsupported. Report the blocker as soon as it is established,
before implementation planning or code-quality review. Continue read-only
investigation and design alternatives. A limitation paragraph, future-work label
or successful unit tests do not close it.

**READY:** each required outcome has a supported causal path and a falsifiable
validation plan; remaining uncertainty cannot invalidate that outcome. An
explicitly approved narrower slice may proceed with its uncovered consequences
stated. Missing telemetry alone is not a blocker when the independent fallback
already meets the requirement.

Keep task-specific tables and evidence in ignored `.agent-work/`; publish accepted
architecture in ADRs. Carry unresolved design blockers into the PR description
and review verdict instead of reporting only implementation correctness.

## Independent validation of this gate

For a requested independent check, start a fresh-context subagent with the
requirement, pinned design/source and these instructions. Supply no expected
finding, diagnosis, prior review verdict or previous chat. Have it report its
first decisive finding before a broader review. Record the exact prompt, source
revision, report and any context contamination in the task evidence. Check that
it also accepts a design with a justified fallback; detecting a defect must not
depend on blocking every design with incomplete telemetry.

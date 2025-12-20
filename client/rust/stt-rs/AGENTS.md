# Codex Skills Bootstrap (agent-dev-cycle)

If ~/.codex/skills/generated/codex-header.md is missing or out of date with AGENTS.md,
the agent MUST stop and regenerate it before proceeding.

### Available skills
Codex has the following skills installed for this repo’s autonomous dev loop:

- **agent-dev-cycle**: orchestrate reconcile → plan → implement → merge → repeat using the four sub-skills
- **reconcile-agent-state**: reconcile `.agent/agent-state.json` with GitHub and compute phase (`idle|planning|implementing|merging`)
- **ensure-issue-tasks**: ensure each issue has a `## Tasks` checklist and a deterministic branch name
- **implement-one-task**: implement exactly ONE task and sync completion back to GitHub
- **merge-and-archive**: merge PRs, close issues, archive state, reset/continue

### Skill selection policy (always follow)
When starting or resuming, ALWAYS run skills in this order:

1) **reconcile-agent-state**
   - If phase becomes `idle`: stop and report “no open work”.
   - If phase becomes `planning`: go to step 2.
   - If phase becomes `implementing`: go to step 3.
   - If phase becomes `merging`: go to step 4.

2) **ensure-issue-tasks**
   - Apply to the next selected open issue(s) until each has a usable `## Tasks` checklist
   - Record `feat/issue-<number>-<slug>` in state
   - Then return to step 1 (**reconcile-agent-state**) to recompute phase.

3) **implement-one-task**
   - Do exactly ONE smallest incomplete task for the current issue.
   - Commit + push + tick the matching GitHub checkbox.
   - If all tasks for the issue are complete, ensure a PR exists.
   - Then return to step 1 (**reconcile-agent-state**) to recompute phase.

4) **merge-and-archive**
   - Merge only when checks/reviews allow; prefer auto-merge if gated.
   - Close issues and archive `.agent` state snapshot.
   - Then return to step 1 (**reconcile-agent-state**) to see if new work exists.

### Recursion rule (keep going)
After completing any skill (except when `idle`), the agent MUST immediately proceed to the next required skill per the policy above, repeating until:
- phase is `idle`, OR
- a stop condition is hit (see below).

### Stop conditions (human required)
STOP and request user input if any of the following occur:
- `gh auth status` fails or repo access is missing
- task intent is ambiguous or unsafe to infer
- tests fail and require product/design decisions (not a straightforward fix)
- merges are blocked by required reviews/checks the agent cannot satisfy

### Hard boundaries
- Do not modify anything under `.github/workflows/`
- One task per `implement-one-task` invocation (no batching)
- GitHub Issues/PRs are the source of truth; `.agent/` is only a resume cache

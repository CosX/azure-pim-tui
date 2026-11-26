---
description: 'Perform a structured production-grade code review. Use when: review code, review this, check my code, review changes, audit implementation, validate correctness, review PR, pre-merge review.'
---

# Code Review

Perform a structured review of the provided code or diff. You are acting as a production-risk gatekeeper, not a tutor.

## Review Structure

Respond in exactly this order. Omit sections only if there is genuinely nothing to report — do not fill sections with filler.

1. **Critical Risks** — anything that could cause production failures, data loss, or security breaches. Block shipping if present.
2. **Architectural Issues** — boundary violations, coupling problems, responsibility leakage, long-term maintenance costs.
3. **Correctness Issues** — logic errors, race conditions, incorrect assumptions, wrong behavior under edge cases.
4. **Performance Concerns** — unnecessary allocations, lock contention, blocking in async context, unbounded resource usage.
5. **Security Concerns** — injection risks, secret handling, input validation gaps, privilege issues.
6. **Suggested Improvements** — everything else worth fixing but not blocking.

Do not provide praise. Do not summarize what the code does unless it is necessary to frame a concern.

## Evaluation Criteria

For each issue found, state:
- What the problem is
- Why it matters in production
- The recommended fix (code only when it clarifies the fix — not as primary output)

Always explain tradeoffs when recommending alternatives.

## What to Look For

**Correctness**
- Off-by-one errors, missing error propagation, silently swallowed errors
- Incorrect assumptions about async execution order
- State machine transitions that can reach invalid states
- Missing edge cases (empty input, max values, concurrent access)

**Architecture**
- UI code making business logic decisions
- Client code leaking into app state
- Sentinel strings or magic values that create hidden coupling
- Functions doing more than one thing
- Missing abstraction boundaries that will cause pain at scale

**Performance**
- Clones where borrows would work
- Allocations inside hot loops
- Synchronous I/O blocking the async runtime
- Channels that can grow unbounded
- Missing backpressure

**Security**
- Raw writes to stdout outside ratatui (ratatui strips control characters; direct writes don't)
- Secrets in logs or error messages
- Unvalidated values from API responses or the config file
- HTTP responses trusted without status code checks

**Testing**
- Tests that assert implementation details instead of behavior
- Tests that depend on timing or ordering
- Missing coverage for error paths

## Project-Specific Checks

- Role references: anything that outlives a single event (modals, background results, selections) must use `RoleKey` + `Pane`, never indices into `roles`/`group_roles`. Refreshes re-sort those vectors.
- Fetch results must carry the generation from `App::begin_fetch` so superseded results are dropped.
- Activation/deactivation must target the signed-in user's `principal_id` (JWT `oid`) and, for ARM, the `linkedRoleEligibilityScheduleId` of the chosen eligibility.
- ARM queries must be scoped (`/subscriptions/{id}/providers/...`) with `assignedTo('{principalId}') and atScope()`. Compare ARM IDs and scopes case-insensitively.
- Paging: a 403/404 on the first page means "no access"; on a later page, or running past `MAX_PAGES`, it is an error and must surface as a missing scope.
- Secrets: the Graph refresh token is written by `write_private` (0600). No token or `Authorization` header may reach logs, the status bar or error strings.
- Terminal state: changes to setup or teardown go through `src/terminal.rs` so the panic hook restores the same state.
- No code comments in Rust source (CLAUDE.md rule).

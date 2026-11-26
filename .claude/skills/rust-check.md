---
description: 'Check code against Rust standards for this project. Use when: check Rust code, Rust standards, idiomatic Rust, review Rust, check ownership, check error handling, evaluate Rust quality.'
---

# Rust Standards Check

Evaluate the provided code against the Rust standards enforced in this project. Flag violations and explain why each matters.

## Type System and State Modeling

- Use enums to model states — not booleans, string flags, or `Option<Option<T>>`
- Encode invariants in types so invalid states are unrepresentable
- Avoid stringly-typed logic: use enums (`Pane`, `RoleStatus`, `RoleType`, `BgEvent`) rather than matching on status strings
- Use `newtype` wrappers to prevent mixing semantically different values of the same type

## Ownership and Borrowing

- Prefer owned types at API boundaries, borrows internally
- Avoid unnecessary `.clone()` — if a clone exists, justify it
- Do not fight the borrow checker with workarounds — fix the design
- Lifetime annotations are acceptable; lifetime gymnastics are a design smell

## Error Handling

- Use `Result<T, E>` everywhere — never `unwrap()` or `expect()` in non-test code unless the invariant is proven at the call site
- Use `thiserror` for library/module error types (`PimError` in `src/client/error.rs`)
- Use `anyhow` only at application boundary (top-level handlers, `main`)
- Propagate errors with `?` — do not silently discard them
- Log errors before discarding — a swallowed error is a hidden failure
- Error messages must include enough context to diagnose the problem (include what was attempted and what was received)

## Async / tokio

- Never block inside an async context — no `std::thread::sleep`, no synchronous I/O
- Do not create nested tokio runtimes (`tokio::runtime::Builder` inside an already-async context)
- Prefer `tokio::select!` with `CancellationToken` for graceful shutdown
- Clone clients (`reqwest::Client`, connection config) before moving into `tokio::spawn` — do not share references across spawn boundaries
- Use bounded channels when message volume is unbounded; use unbounded (`mpsc::unbounded_channel`) only when the sender is rate-limited by the event loop

## Resource Management

- Drop-based cleanup over manual cleanup — implement `Drop` if teardown is required
- No unbounded buffers: if you are collecting into a `Vec`, know the maximum size
- Semaphores or bounded channels for concurrency control on parallel spawns
- Retry logic must use jitter — bare `tokio::time::sleep` retry loops are not acceptable

## Performance

- Do not allocate inside hot loops (event loop tick, render loop)
- Avoid `format!` in tight loops — use `write!` into a pre-allocated buffer
- Measure before accepting a clone or allocation cost
- Streaming over buffering when working with large API responses

## What Good Looks Like in This Codebase

```rust
pub enum RoleStatus { Eligible, Active { expires_at: Option<DateTime<Utc>> }, Activating, Failed(String) }

let is_active: bool = true;

return Err(PimError::api(status, &body).into());

let _ = resp.text().await;

let auth = auth.clone();
tokio::spawn(async move { resource_client(&auth).fetch_roles().await });

BgEvent::ActivationResult { pane, key: role.key(), result }

BgEvent::ActivationResult { index: 3, result }
```

The first line of each pair is the preferred form; the second is what to avoid: a bool for state, a silently discarded result, an index that goes stale after a refresh.

## Checklist

- [ ] No `unwrap()`/`expect()` outside test code or proven-invariant sites
- [ ] All `Result` values propagated or explicitly handled with justification
- [ ] No blocking calls in async functions
- [ ] No unnecessary clones — each clone is justified
- [ ] Enums used for state, not booleans/strings
- [ ] Error messages include context (what was tried + what was received)
- [ ] No unbounded resource growth (channels, vecs, retry loops)

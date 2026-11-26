---
description: 'Evaluate code for security risks. Use when: security review, security audit, check for vulnerabilities, injection risk, input validation, secret handling, check security, hostile input.'
---

# Security Check

This app activates privileged Azure roles for the signed-in user. The main risks are acting on the wrong role, leaking tokens, and trusting API responses too much.

## Privileged actions

- Activation and deactivation must act on exactly the role the user confirmed. Modals and results hold `RoleKey`, re-resolved at submit time. Any index-based reference into role vectors is a Critical finding.
- `principalId` in request bodies must come from the JWT `oid` of the signed-in user, never from API response data.
- Bulk activation may only include roles visible in the current filtered view.

## Secrets

- Tokens: management tokens live in memory (`CachedTokenCredential`). The Graph token and refresh token are cached at `<config_dir>/azure-pim-tui/graph_token.json` and must be written with `write_private` (mode 0600 on Unix, also fixed on existing files).
- Never log tokens, `Authorization` headers or token endpoint response bodies. Errors from the token endpoint may be logged by `error`/`error_description` only.
- The log file (`azure-pim-tui.log`) rotates at 5MB. Don't add per-request logging at `info` or above.

## Input and output

- OData `$filter` values are built from the JWT `oid` and IDs returned by Graph. Never interpolate user-typed text into a filter or URL.
- Justification text is sent as a JSON string field via `serde`. Don't build JSON by hand.
- ratatui drops control characters when drawing, so API strings are safe to render inside widgets. Anything written to stdout directly (`src/terminal.rs`) must be a constant escape sequence.

## HTTP

- Check `status()` before parsing a body. Build API errors with `PimError::api(status, &body)` so the UI shows `code: message`, not raw JSON.
- First-page 403/404 means "no access, no roles". Later-page failures and `MAX_PAGES` overflow are errors and must not be reported as complete results.
- All requests go through the shared client from `client::http::new_client()`, which has connect and request timeouts.

## Checklist

- [ ] No index-based role references in modals or background events
- [ ] No secrets in logs, status messages or error strings
- [ ] Token cache written owner-only
- [ ] No user input in URLs or OData filters
- [ ] Status checked before body parsing; partial results flagged

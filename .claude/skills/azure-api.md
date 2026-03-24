---
description: 'Add a new Azure API call (ARM or Graph). Use when: new API endpoint, new REST call, add Azure Management API method, add Graph API method, new PIM endpoint, fetch data from Azure, call ARM API, call Microsoft Graph.'
---

# Add Azure API Endpoint

Adds a new REST API call to the ARM (Azure Resource Manager) or Microsoft Graph client layer.

## Two API Surfaces

| Surface | Client | Auth | Base URL | Scope |
|---------|--------|------|----------|-------|
| **ARM** (resource roles) | `PimClient` in `client/pim.rs` | `DeveloperToolsCredential` (az CLI) | `https://management.azure.com` | `https://management.azure.com/.default` |
| **Graph** (groups, Entra ID) | `GroupPimClient` in `client/group.rs` | `GraphCredential` (device code flow, cached) | `https://graph.microsoft.com/v1.0` | PIM-specific scopes via device code |

Both clients follow the same pattern: `reqwest::Client` + credential + `get_token()` per request.

## Files Touched (in order)

| Step | File | What |
|------|------|------|
| 1 | `src/client/models.rs` | Add serde structs for request/response bodies |
| 2 | `src/client/pim.rs` or `src/client/group.rs` | Add async method to the client |
| 3 | `src/app.rs` | Add `BgEvent` variant if this is a new background operation |
| 4 | `src/main.rs` | Spawn the task + handle the `BgEvent` result |

## Step 1: Add Models

In `src/client/models.rs`, add response structs with `Deserialize` and request structs with `Serialize`.

**ARM API conventions:**
- Responses wrap items in `{ "value": [...] }` — use the existing `ApiListResponse<T>` generic
- Properties are nested under a `properties` field
- Use `#[serde(rename_all = "camelCase")]` on property structs

```rust
#[derive(Debug, Deserialize)]
pub struct MyArmResponse {
    pub id: String,
    pub properties: MyArmProperties,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyArmProperties {
    pub some_field: String,
}
```

**Graph API conventions:**
- Responses wrap items in `{ "value": [...] }` — use the existing `GraphListResponse<T>`
- Fields are flat camelCase, no `properties` wrapper
- Use `#[serde(rename_all = "camelCase")]`

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyGraphResponse {
    pub id: String,
    pub display_name: Option<String>,
}
```

## Step 2: Add Client Method

### ARM method (in `src/client/pim.rs`)

```rust
pub async fn my_method(&self, scope: &str) -> Result<Vec<MyArmResponse>> {
    let token = self.get_token().await?;
    let url = format!("{BASE_URL}{scope}/providers/Microsoft.Authorization/myResource");

    let resp = self
        .client
        .get(&url)
        .query(&[("api-version", API_VERSION), ("$filter", &some_filter)])
        .bearer_auth(&token)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(PimError::Api { status, message: body }.into());
    }

    let body: ApiListResponse<MyArmResponse> = resp.json().await?;
    Ok(body.value)
}
```

**ARM rules:**
- Always call `self.get_token().await?` before each request (tokens refresh automatically)
- ARM endpoints are **scoped**: `{BASE_URL}/subscriptions/{id}/providers/...`
- Always pass `api-version` as a query param (currently `2020-10-01`)
- Use `.query()` for URL params — never string-interpolate filter values
- For per-subscription operations, iterate `self.subscriptions` and use `futures::future::join_all` for parallelism

### Graph method (in `src/client/group.rs`)

```rust
pub async fn my_method(&self) -> Result<Vec<MyGraphResponse>> {
    let token = self.get_token().await?;
    let url = format!("{GRAPH_BASE}/myResource");
    let filter = format!("principalId eq '{}'", self.principal_id);

    let resp = self
        .client
        .get(&url)
        .query(&[("$filter", &filter)])
        .bearer_auth(&token)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        // Gracefully handle tenants that don't support this feature
        if status == 400 || status == 403 || status == 404 {
            warn!("API returned {status}: {body}");
            return Ok(vec![]);
        }
        return Err(PimError::Api { status, message: body }.into());
    }

    let body: GraphListResponse<MyGraphResponse> = resp.json().await?;
    Ok(body.value)
}
```

**Graph rules:**
- `GraphCredential` handles device code flow + caching automatically
- Graph has no API version in query params (version is in the URL path: `/v1.0/` or `/beta/`)
- Use `tracing::warn!` for non-fatal API errors so they appear in debug logs
- For 403 (permission denied), return empty rather than erroring — the tenant may not support the feature
- For write operations (activate/deactivate), use POST not PUT (unlike ARM)

## Step 3: Add BgEvent (if new background operation)

In `src/app.rs`, add a variant to the `BgEvent` enum:

```rust
pub enum BgEvent {
    // ... existing ...
    MyOperationResult(Result<SomeData, String>),
}
```

Then handle it in `handle_bg_event()`. Remember to handle both `Pane::Resources` and `Pane::Groups` if the operation applies to both:

```rust
BgEvent::MyOperationResult(Ok(data)) => {
    // Update state for the correct pane
    match self.active_pane {
        Pane::Resources => { /* ... */ }
        Pane::Groups => { /* ... */ }
    }
    self.update_filtered_indices();
}
```

## Step 4: Spawn from main.rs

Resource pane operations are spawned from `spawn_fetch_resources()` or `handle_modal_action()`. Group pane operations from `spawn_fetch_groups()` or `handle_modal_action()`.

```rust
tokio::spawn(async move {
    let result = if role.role_type == RoleType::Resource {
        let client = PimClient::new(auth.credential.clone(), ...);
        client.my_method(&role.scope).await
    } else {
        let client = GroupPimClient::new(auth.graph_credential.clone(), ...);
        client.my_method().await
    };
    let _ = tx.send(BgEvent::MyOperationResult(result.map_err(|e| e.to_string())));
});
```

## Error Handling

Use `PimError` variants from `src/client/error.rs`:

| Variant | When |
|---------|------|
| `PimError::Auth(msg)` | Token acquisition fails |
| `PimError::Api { status, message }` | Non-success HTTP status |
| `PimError::RoleAssignmentExists` | 409-like: already active |
| `PimError::Other(msg)` | Business logic errors |

Known API error strings to detect in response bodies:
- `RoleAssignmentExists` — role already activated
- `ActiveDurationTooShort` — activated too recently to modify/deactivate

## Checklist

- [ ] Response/request serde structs added to `models.rs`
- [ ] Client method calls `self.get_token().await?` before each request
- [ ] ARM calls include `api-version` query param
- [ ] ARM calls use scoped URLs (not tenant-level)
- [ ] Graph calls gracefully handle 403/404 (return empty, not error)
- [ ] URL params use `.query()` — no string interpolation of user input
- [ ] `BgEvent` variant added and handled in `handle_bg_event()`
- [ ] Spawn block clones client + `bg_tx` before `move`
- [ ] Error path maps to `String` via `.map_err(|e| e.to_string())`

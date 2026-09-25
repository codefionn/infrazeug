# ZITADEL resources

`infrazeug-zitadel` adds four native ensure methods: `ensure_project`, `ensure_oidc_app`, `ensure_project_role`, and `ensure_human_user`. They adopt objects by project name, app name within a project, role key within a project, and username. The client is `infrazeug-ext-zitadel-api`.

Use `client_from_env()` with `ZITADEL_URL` and `ZITADEL_TOKEN`, or use `ZitadelInfraExt::zitadel_vault("path/to/file", machine_id)` with `base_url` and `token` fields in an unlocked controller vault file. The token must have permission to read and change the managed organization. The API client sends it as a bearer token and does not refresh it. A personal access token is a straightforward choice.

```rust,ignore
use infrazeug_zitadel::{client_from_env, EnsureProjectInput, ZitadelInfraExt};

let bundle = builder
    .zitadel(client_from_env()?, controller)
    .ensure_project(project_node, "identity-project", EnsureProjectInput {
        name: "identity".into(),
        ..Default::default()
    })?
    .finish();
```

Pass the project node as a dependency to `ensure_oidc_app` and `ensure_project_role`. Set their `project_id` to `ResourceInput::node(project_node.0).json_pointer("/id")` so it resolves from the project capture, or use `ResourceInput::inline(existing_id)`. The OIDC app output includes its generated `client_secret` only on creation; ZITADEL does not return the existing secret on reads. Capture it to a mutable vault in the same run. Later observations return `None` for that field. If creation succeeds but the run fails before capture, generate a new secret in ZITADEL and store it in the vault.

The implementation uses ZITADEL Management v1 REST routes. ZITADEL marks these routes deprecated in favor of resource v2 services. The v1 routes are used here because they provide one coherent JSON REST API for all four resource types. The methods reconcile only declared fields. OIDC config and human profile updates fetch the current representation before PUT so fields outside the managed set survive.

Official API definitions: [projects](https://zitadel.com/docs/reference/api/management/zitadel.management.v1.ManagementService.AddProject), [OIDC applications](https://zitadel.com/docs/reference/api/management/zitadel.management.v1.ManagementService.AddOIDCApp), [OIDC config updates](https://zitadel.com/docs/reference/api/management/zitadel.management.v1.ManagementService.UpdateOIDCAppConfig), [project roles](https://zitadel.com/docs/reference/api/management/zitadel.management.v1.ManagementService.AddProjectRole), [human users](https://zitadel.com/docs/reference/api/management/zitadel.management.v1.ManagementService.AddHumanUser).

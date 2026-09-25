# Authentik resources

`infrazeug-authentik` adds four native `EnsureResource` methods. Each method observes by a stable key, creates an absent object, compares only the fields declared in the input, and updates changed fields with PATCH. The methods run on the controller machine.

| Method | Lookup key | Managed fields |
| --- | --- | --- |
| `authentik.ensure_user` | exact username | name, optional email, active state, path, group UUIDs |
| `authentik.ensure_group` | exact name | optional superuser state, parent UUID, user IDs |
| `authentik.ensure_application` | slug | name, optional provider ID or OAuth2 provider name, description, publisher |
| `authentik.ensure_oauth2_provider` | exact name | required authorization and invalidation flow UUIDs, optional client ID, client type, redirect URIs |

For Authentik client credentials, use `client_from_env()` with `AUTHENTIK_URL` and `AUTHENTIK_TOKEN`, or use `AuthentikInfraExt::authentik_vault(file, machine_id)`. A vault file needs `base_url` and `token` fields. Vault credentials are read on first use during apply. A read-only plan without an unlocked vault reports these resources as unknown.

```rust,ignore
use infrazeug_api::builder::InfraBuilder;
use infrazeug_authentik::{AuthentikInfraExt, EnsureOAuth2ProviderInput, EnsureApplicationInput};

let bundle = InfraBuilder::new()
    // Add the controller machine and any earlier nodes here.
    .authentik_vault("auth/authentik.vault", controller_id)
    .ensure_oauth2_provider(provider_node, "web-oidc", EnsureOAuth2ProviderInput {
        name: "web-oidc".into(),
        authorization_flow: authorization_flow_uuid.into(),
        invalidation_flow: invalidation_flow_uuid.into(),
        ..Default::default()
    })?
    .ensure_application_after(application_node, "web", EnsureApplicationInput {
        slug: "web".into(),
        name: "Web".into(),
        provider_name: Some("web-oidc".into()),
        ..Default::default()
    }, [provider_node])?
    .finish();
```

The OAuth2 provider output includes `client_secret` when Authentik returns one. It is captured for downstream vault writes, as with Keycloak's confidential client. The input does not contain a secret, so Authentik generates it. Keep node captures out of logs. The Rust `Debug` output redacts this field. If a server omits the secret in both the list and detail responses, the output field is `None`.

`None` means "leave an optional field alone". Set an empty list to remove managed memberships or redirect URIs. The current inputs do not express clearing an optional scalar such as an application's provider assignment or a group's parent. If an Authentik instance allows duplicate group or provider names, the method returns an error rather than selecting one arbitrarily. Use `provider_name` with `ensure_application_after` to wire a provider created in the same run. `provider` accepts a known numeric provider ID for other cases. If both are set, `provider_name` takes precedence.

API reference: [users](https://api.goauthentik.io/reference/core-users-create/), [groups](https://api.goauthentik.io/reference/core-groups-create/), [applications](https://api.goauthentik.io/reference/core-applications-create/), [OAuth2 providers](https://api.goauthentik.io/reference/providers-oauth-2-create/), [OAuth2 provider detail](https://api.goauthentik.io/reference/providers-oauth-2-retrieve/), and [API overview](https://api.goauthentik.io/).

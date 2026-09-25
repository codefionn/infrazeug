# Authentik API client

`AuthentikClient::new(base_url, token)` accepts an instance root such as `https://auth.example` or a URL ending in `/api/v3/`. It sends the token as an HTTP Bearer credential. The client exposes list, detail, create, and PATCH methods for users, groups, applications, and OAuth2 providers. List calls follow every page and filter results by exact key in the resource crate.

This is the low-level HTTP crate. For `AUTHENTIK_URL` and `AUTHENTIK_TOKEN`, vault credentials, native nodes, and generated OAuth2 client secret capture, use [`infrazeug-authentik`](../infrazeug-authentik/README.md).

The paths, request fields, and response IDs follow the official Authentik API reference for [users](https://api.goauthentik.io/reference/core-users-create/), [groups](https://api.goauthentik.io/reference/core-groups-create/), [applications](https://api.goauthentik.io/reference/core-applications-create/), and [OAuth2 providers](https://api.goauthentik.io/reference/providers-oauth-2-create/). Authentik instances expose the API browser at `/api/v3/` according to the [API overview](https://api.goauthentik.io/).

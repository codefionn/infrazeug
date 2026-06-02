# hello-ovh

Ensures an OVH Public Cloud S3 Object Storage bucket, S3 user, and bucket policy exist, then stores credentials in the **mutable vault** using standard tier-2 `VaultWrite` shell nodes.

## Graph shape

```text
native: ensure bucket
  → native: ensure S3 user  (JSON capture)
    → native: ensure S3 user policy
      → shell: VaultWrite credentials.access_key   (capture + json_pointer)
      → shell: VaultWrite credentials.secret_key
```

Native nodes publish JSON captures (like shell stdout). Vault nodes use the same `mutable_vault_write` + `FileSource::capture` path as every other playbook.

## Credentials from the vault

The OVH API credentials are **read from the vault at apply time**, not from the
environment. `init` stores them once (sealed under DataKey `prod-runtime` in
`vault-store/files/cloud/ovh.vault`); `apply` reads them back through the
controller's unlocked vault session via `OvhInfraExt::ovh_vault`. No `OVH_*` secret
needs to be exported when applying.

## S3 Object Storage vs Swift

OVH has two object-storage surfaces that look similar but use different APIs:

| Surface | OVH API | Access model | infrazeug support |
|---------|---------|--------------|-------------------|
| S3 Object Storage | `/cloud/project/{project}/region/{region}/storage` | Public Cloud user + `s3Credentials` + user policy | `BackupStack`, `ensure_storage_container`, `ensure_s3_user`, `ensure_s3_user_policy` |
| Legacy Swift/Object Storage | `/cloud/project/{project}/storage` | OpenStack/Swift credentials and ACLs | not provisioned by `BackupStack` |

`BackupStack` is for the S3-compatible Object Storage product. It intentionally uses the region-scoped storage API and creates an S3 credential for a Public Cloud user. Do not use it for legacy Swift containers.

S3 buckets are private unless the S3 user policy grants access. Creating a Public Cloud user and issuing `s3Credentials` is not enough by itself for backup software such as CloudNativePG/Barman. `BackupStack` therefore also applies an S3 user policy that grants read/write access to:

```text
arn:aws:s3:::<container>
arn:aws:s3:::<container>/*
```

The policy includes the S3 actions needed by backup tooling: bucket listing/location, object get/put/delete, multipart listing, multipart abort, and multipart upload-part listing.

## Setup

Provide the API credentials once so `init` can seal them into the vault:

```bash
export OVH_APPLICATION_KEY=... OVH_APPLICATION_SECRET=... OVH_CONSUMER_KEY=...
export OVH_ENDPOINT=eu   # optional: eu (default) / us / ca
cargo run -p hello-ovh -- init
```

`init` is re-runnable: it unlocks the existing store (passphrase `demo`) and
re-stores whatever credentials are in the environment.

## Apply

```bash
export OVH_PROJECT_ID=...            # project id is config, not a secret
export OVH_CONTAINER_NAME=...        # optional, default infrazeug-backups
cargo run -p hello-ovh -- apply --tui   # enter passphrase `demo` to unlock the vault
```

## Environment

| Variable | When | Notes |
|----------|------|-------|
| `OVH_APPLICATION_KEY` | `init` | sealed into the vault |
| `OVH_APPLICATION_SECRET` | `init` | sealed into the vault |
| `OVH_CONSUMER_KEY` | `init` | sealed into the vault |
| `OVH_ENDPOINT` | `init` | optional (`eu` / `us` / `ca`) |
| `OVH_PROJECT_ID` | `apply` | required (identifier, not a secret) |
| `OVH_CONTAINER_NAME` | `apply` | optional (default `infrazeug-backups`) |

Generated S3 credentials land in `vault-store/files/mutable/cloud/<container>.vault`
under DataKey `prod-runtime`.

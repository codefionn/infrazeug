# Vault store format

Infrazeug’s encrypted secret store lives under a single directory (the **store root**). The layout and wire formats are versioned; opening a store runs **automatic, idempotent migrations** so older on-disk shapes upgrade in place.

## Store layout (format version 3)

```
<store>/
  meta/
    store.cbor              # store-level format version
  keys/
    <data_key_id>.dkey      # DataKey envelope (wrapped; see below)
  files/
    <path>.vault            # encrypted CBOR secret map
    mutable/
      <path>.vault          # generated mutable secret map
  packs/                    # optional bundles (high-latency backends)
    <name>.pack
```

| Path | Purpose |
|------|---------|
| `meta/store.cbor` | CBOR `StoreMeta { format_version }`. Written on first use; currently `3`. |
| `keys/*.dkey` | One file per named DataKey (`prod`, `ops`, …). |
| `files/**/*.vault` | Secret payloads sealed under a DataKey. |
| `files/mutable/**/*.vault` | Reserved namespace for generated secrets written by runs. Same vault-file wire format; no separate plaintext state file. |

**Store without `meta/store.cbor`** is treated as format **v0** (legacy). On first `VaultStore` operation, migrations run:

1. Wrap any bare-CBOR `.dkey` files in the `INFRZDK1` wire header (no secrets required).
2. Remove duplicate recipient labels per DataKey (keeps the first; clears envelope `auth` so it is re-sealed on next unlock).
3. Write `meta/store.cbor` with the current `format_version` (`3`).

Format v3 hardens backend key handling: backend keys must be store-relative,
must not contain `..`, empty path components, backslashes, NUL, or URL
metacharacters, and the local filesystem backend writes private files/directories
by default. Opening a v2 store also rewrites existing v1 vault-file wrappers to
the current v2 vault-file version byte without decrypting payloads.

## Key model

```
recipients (passphrase / FIDO2 / PKCS#11 / age / KMS / …)
     │ wrap DEK
     ▼
DataKey (32-byte random, named id)
     │ unlock via any one recipient
     ▼
DEK in memory (zeroized when the run ends)
     │ XChaCha20-Poly1305
     ▼
Vault file (CBOR map of secrets)
```

- **Onboard device**: add a recipient to the `.dkey` envelope; vault file bodies unchanged.
- **Recipient labels** are unique per DataKey (used by the CLI and unlock).

## DataKey envelope (`keys/<id>.dkey`)

### Wire format (v1)

```
offset  size  field
0       8     magic  "INFRZDK1"
8       1     wire_version  0x01
9       4     inner_len (u32 BE)
13      N     inner CBOR (see below)
```

**Legacy (v0)** stores `inner` CBOR directly (no magic). Loaders accept both; migration rewrites to v1 wire.

### Inner CBOR (`DataKeyFile`)

```rust
{
  "envelope": {
    "id": "<data_key_id>",
    "file_salt": <32 bytes>,
    "recipients": [ RecipientEntry, ... ],
    "auth": <nonce24 || tag16>   // empty on legacy; sealed after first unlock
  }
}
```

`RecipientEntry`:

| Field | Type | Notes |
|-------|------|--------|
| `kind` | enum | `Passphrase`, `Fido2`, `Pkcs11`, `Age`, `Kms`, `SshAgent` |
| `label` | string | Unique per DataKey |
| `wrapped_key` | bytes | Provider-specific sealed DEK material |
| `params` | JSON object | Provider metadata (e.g. FIDO credential id, Argon2 params) |

**Envelope authentication (`auth`)**: XChaCha20-Poly1305 over the canonical CBOR of the envelope with `auth` cleared, keyed by `SHA-256("infrazeug-envelope-auth-v1" || DEK)`. Detects recipient tampering or reordering. Legacy envelopes without `auth` are upgraded automatically on the first successful unlock.

## Vault file (`files/.../*.vault`)

```
offset  size  field
0       8     magic  "INFRZVLT"
8       1     version  0x02
9       4     header_len (u32 BE)
13      H     header CBOR
13+H    …     ciphertext (XChaCha20-Poly1305)
```

### Header CBOR (`VaultHeader`)

| Field | Type |
|-------|------|
| `data_key_id` | string |
| `content_type` | string (e.g. `application/cbor`) |
| `nonce` | 24 bytes |
| `aad_hash` | SHA-256 of canonical header with `aad_hash` zeroed (kept for format compatibility; integrity is enforced by the AEAD tag, which covers the whole header as AAD — there is no separate hash check on read) |
| `file_salt` | 32 bytes |

**Plaintext body**: CBOR map (`string` keys; values: nested maps, lists, bytes, strings, ints, bools, null). Field paths use dot notation (`db.host`, `items.0`).

**AEAD**: XChaCha20-Poly1305(DEK, plaintext, nonce, AAD = canonical header bytes).

### Mutable namespace

`files/mutable/` is a reserved convention for secrets created or updated by
infrazeug runs, for example cloud bucket-scoped API keys minted after creating a
bucket. It uses the same encrypted vault-file format and DataKey model as any
other vault file; the distinction is operational, not cryptographic.

Typical use:

- generated object: `files/mutable/cloud/<bucket>.vault`
- DataKey scope: a runtime key such as `prod-runtime`, separate from human-authored
  static secrets if a narrower blast radius is useful
- fields: `bucket.name`, `credentials.access_key`, `credentials.secret_key`

Mutable files remain secret material. MCP must treat them under the same
metadata-only rule as normal vault files.

ShellOps can populate mutable vault fields through a controller-side
`VaultWrite` op. A common flow is: run the cloud CLI, capture stdout from that
node, apply source transforms such as regex include/exclude and trim, then write
the resulting bytes into a mutable vault field.

## Migration summary

| From | To | When | Requires unlock |
|------|-----|------|-----------------|
| Store v0 (no meta, bare `.dkey`) | Store v1 | First store access | No |
| Duplicate recipient labels | Unique labels (first wins) | Store open (v1→v2) | No |
| Store v2 | Store v3 | Store open; vault-file wrappers are bumped to v2, backend key validation and private FS writes apply | No |
| Envelope without `auth` | Authenticated envelope | First successful unlock | Yes |
| Vault file v1 | Read-compatible legacy | N/A | - |
| New vault writes | Vault file v2 | Every write with current code | Yes |

Future vault-file versions can add new `version` bytes; loaders should migrate or reject explicitly.

## Security considerations

- **Memory hygiene**: DEKs and passphrases are held in `Zeroizing` wrappers and
  scrubbed on drop / `lock_all`. The decrypted file cache stores canonical CBOR
  plaintext in `Zeroizing<Vec<u8>>` for the same reason. Decoded value maps
  handed to callers are ordinary heap data and are not zeroized.
- **No memory locking**: keys and plaintext are not `mlock`ed; they can reach
  swap or core dumps on a compromised or misconfigured host. Run on hosts with
  encrypted swap / disabled core dumps if that is in your threat model.
- **Writes**: the local backend writes via O_EXCL temp files (mode `0600`,
  dirs `0700`), fsyncs the payload, renames atomically, and fsyncs the parent
  directory. CLI `vault edit` stages plaintext in a private `0700` temp dir.
- **Legacy envelopes** without `auth` are trusted until the first unlock
  re-seals them; treat stores from untrusted writers accordingly.

## Related code

- `infrazeug-secrets::store_format` — constants and `StoreMeta`
- `infrazeug-secrets::migrate` — `ensure_store_format`, `migrate_envelope_after_unlock`
- `infrazeug-secrets::envelope` — `encode_dkey_blob` / `decode_dkey_blob`
- `infrazeug-secrets::format` — vault file encrypt/decrypt

Design authority: [SOUL.md](../SOUL.md) section 6.

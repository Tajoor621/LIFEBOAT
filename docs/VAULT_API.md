# LIFEBOAT Vault API — P1.2

This is an early authenticated transport boundary for **already encrypted** objects. It is not a production-ready public Internet service.

## Security boundary

- The API stores opaque ciphertext and encrypted manifest bytes. It has no plaintext decryption endpoint and no backup master key.
- Every data route is scoped to a 128-bit device identifier and requires that device's bearer token.
- Device credentials are provisioned by the vault operator through `LIFEBOAT_DEVICE_TOKENS`; there is no public self-registration endpoint.
- The server retains BLAKE3 hashes of high-entropy bearer tokens in process memory and compares them in constant time. Use randomly generated tokens of at least 32 bytes. Do not use human passwords.
- Each device has a separate storage directory. An authenticated device cannot address another device's object namespace by changing the URL.
- Object and manifest identifiers are lowercase hexadecimal only. Storage is immutable: duplicate PUT returns HTTP 409 rather than overwriting.
- Upload responses return `x-lifeboat-content-blake3`. GET requests must send that digest in the same header; the store verifies bytes against it before returning data.
- Request bodies are bounded. Objects are limited to 16 MiB + 16 bytes (ciphertext plus the current AEAD tag); encrypted manifests are limited to 8 MiB.
- Operational logs include device and opaque object identifiers, byte counts, and action names. They never include bearer tokens or payload bytes.

## Configuration

Required:

- `LIFEBOAT_DEVICE_TOKENS`: semicolon-separated `device_id:token` entries. Device IDs are exactly 32 lowercase hex characters; tokens must be at least 32 bytes. Example shape only: `00112233445566778899aabbccddeeff:<random-40-byte-or-longer-secret>`.
- `LIFEBOAT_VAULT_ROOT`: storage directory. Defaults to `./lifeboat-vault`.

Optional:

- `LIFEBOAT_BIND`: socket address. Defaults to `127.0.0.1:8080`. Keep the default for local use. A container deployment may bind inside its private container network and publish only to host loopback.

Generate credentials with a cryptographically secure random generator. Keep them in a host secret manager or root-readable environment file, never in Git, mobile source code, URLs, shell history, screenshots, or support logs. Rotate by replacing the configured token and restarting the service; current P1.2 has no online revocation or rotation endpoint.

## Routes

All routes except health require `Authorization: Bearer <device-token>`.

| Method | Path | Purpose |
|---|---|---|
| GET | `/healthz` | Process health only; reveals no vault data |
| PUT | `/v1/devices/{device_id}/objects/{object_id}` | Store immutable ciphertext |
| GET | `/v1/devices/{device_id}/objects/{object_id}` | Retrieve ciphertext after digest verification |
| PUT | `/v1/devices/{device_id}/manifests/{snapshot_id}` | Store immutable encrypted manifest |
| GET | `/v1/devices/{device_id}/manifests/{snapshot_id}` | Retrieve encrypted manifest after digest verification |

For GET, supply `x-lifeboat-content-blake3: <64 lowercase hex characters>`. PUT returns this header with the stored BLAKE3 digest. The API does not interpret object IDs, snapshot contents, file names, or data classes.

## Example

```sh
curl --fail-with-body \
  -H "Authorization: Bearer $LIFEBOAT_DEVICE_TOKEN" \
  -H "Content-Type: application/octet-stream" \
  --data-binary @encrypted-chunk.bin \
  -D response-headers.txt \
  -X PUT "http://127.0.0.1:8080/v1/devices/$DEVICE_ID/objects/$OBJECT_ID"

# Copy x-lifeboat-content-blake3 from response-headers.txt:
curl --fail-with-body \
  -H "Authorization: Bearer $LIFEBOAT_DEVICE_TOKEN" \
  -H "x-lifeboat-content-blake3: $CONTENT_DIGEST" \
  "http://127.0.0.1:8080/v1/devices/$DEVICE_ID/objects/$OBJECT_ID" \
  -o downloaded-ciphertext.bin
```

## Explicit limitations / release gates

P1.2 does **not** yet provide TLS termination, user accounts, device enrollment, token rotation/revocation, per-device quotas, rate limiting, resumable multipart/chunk sessions, append-only tamper-evident audit storage, retention/deletion policy, S3 backend, or network fuzzing. Do not expose the service directly to the public Internet. For remote use, put it behind a correctly configured TLS reverse proxy, restrict network access, and manage secrets outside the repository. The local immutable store still uses filesystem hard-link create-if-absent semantics; filesystem and volume behavior must be validated on each deployment target.

The API is a transport layer only. Client-side encryption, authenticated manifest semantics, backup completeness, and restore verification remain separate responsibilities.

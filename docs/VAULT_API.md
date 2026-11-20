# LIFEBOAT Vault API — P1.2 / P1.3

This is an early authenticated transport boundary for **already encrypted** objects. It is not a production-ready public Internet service.

## Security boundary

- The API stores opaque ciphertext and encrypted manifest bytes. It has no plaintext decryption endpoint and no backup master key.
- Every data route is scoped to a 128-bit device identifier and requires that device's bearer token.
- Device credentials are provisioned by the vault operator through `LIFEBOAT_DEVICE_TOKENS`; there is no public self-registration endpoint.
- The server retains BLAKE3 hashes of high-entropy bearer tokens in process memory and compares them in constant time. Use randomly generated tokens of at least 32 bytes. Do not use human passwords.
- Each device has a separate storage directory. An authenticated device cannot address another device's object namespace by changing the URL.
- Object and manifest identifiers are lowercase hexadecimal only. Storage is immutable: duplicate PUT returns HTTP 409 rather than overwriting.
- Upload responses return `x-lifeboat-content-blake3`. GET requests must send that digest in the same header; the store verifies bytes against it before returning data.
- Authentication middleware runs before handler body extraction. Per-device requests are limited to 120 per 60-second in-memory window.
- Each device has a 50 GiB storage quota. In-flight upload parts count against the quota; session creation is serialized with writes.
- Request bodies are bounded. Objects are limited to 16 MiB + 16 bytes (ciphertext plus the current AEAD tag); encrypted manifests are limited to 8 MiB. Resumable upload chunks are limited to 1 MiB.
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
| HEAD | `/v1/devices/{device_id}/objects/{object_id}` | Check object existence, byte count and verified digest for deduplication |
| PUT | `/v1/devices/{device_id}/manifests/{snapshot_id}` | Store immutable encrypted manifest |
| GET | `/v1/devices/{device_id}/manifests/{snapshot_id}` | Retrieve encrypted manifest after digest verification |
| POST | `/v1/devices/{device_id}/uploads` | Create a persistent fixed-chunk upload session |
| GET | `/v1/devices/{device_id}/uploads/{upload_id}` | Read resumable session progress |
| PUT | `/v1/devices/{device_id}/uploads/{upload_id}/chunks/{index}` | Upload one independently hashed chunk |
| POST | `/v1/devices/{device_id}/uploads/{upload_id}/complete` | Verify full digest and atomically commit immutable object |

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

## Resumable upload protocol

Create a session with JSON containing `object_id` (64 lowercase hex characters), `total_size`, `chunk_size` (64 KiB–1 MiB), and `expected_blake3` (the expected digest of the complete ciphertext object). The server allocates a persistent sparse part file and returns an opaque 128-bit `upload_id`. A device can query the session after reconnect and resume only chunks not yet acknowledged.

Upload chunks with zero-based chunk indexes and `x-lifeboat-chunk-blake3`. Every chunk must have the exact expected length and digest. Repeating a successfully acknowledged chunk with the same digest is idempotent; conflicting retries are rejected. The server syncs chunk bytes before persisting the chunk receipt. Completion requires every chunk receipt and a full-object BLAKE3 match before immutable object commit. A small completion receipt makes completion retry-safe if the response is lost. Incomplete sessions expire after seven days and at most 32 active sessions are allowed per device.

Use HEAD on an object before uploading it: compare the server's `x-lifeboat-content-blake3` and `Content-Length` with the client-computed values. A deduplication hit is valid only when both match.

## Explicit limitations / release gates

P1.2/P1.3 still do **not** provide TLS termination, user accounts, device enrollment, online token rotation/revocation, durable distributed rate limiting, a tamper-evident audit ledger, retention/deletion policy for committed objects, S3 backend, multi-node coordination, or network fuzzing. Do not expose the service directly to the public Internet. For remote use, put it behind a correctly configured TLS reverse proxy, restrict network access, and manage secrets outside the repository. The local immutable store still uses filesystem hard-link create-if-absent semantics; filesystem and volume behavior must be validated on each deployment target.

The API is a transport layer only. Client-side encryption, authenticated manifest semantics, backup completeness, and restore verification remain separate responsibilities. Rate limits and write serialization are process-local and are not suitable for horizontally scaled replicas. A single-process local deployment is assumed for this phase.

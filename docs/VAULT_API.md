# LIFEBOAT Vault API — P1.2 / P1.6

This is an authenticated transport boundary for **already encrypted** objects. It is not a production-ready public Internet service.

## Security boundary

- The API stores opaque ciphertext and encrypted manifest bytes. It has no plaintext decryption endpoint and no backup master key.
- Every data route is scoped to a 128-bit device identifier and requires that device's bearer token.
- Credentials are provisioned offline by the vault operator; there is no public self-registration endpoint.
- The server retains only BLAKE3 digests of high-entropy bearer tokens and compares them in constant time. Raw tokens are not stored in the credentials file or logged.
- Each device has a separate storage directory. An authenticated device cannot address another device's object namespace by changing the URL.
- Object and manifest identifiers are lowercase hexadecimal only. Storage is immutable: duplicate PUT returns HTTP 409 rather than overwriting.
- Upload responses return `x-lifeboat-content-blake3`. GET requests must send that digest in the same header; the store verifies bytes against it before returning data.
- Authentication middleware runs before handler body extraction. Per-device requests are limited to 120 per 60-second in-memory window.
- Each device has a 50 GiB storage quota. In-flight upload parts count against the quota; session creation is serialized with writes.
- Route body limits are enforced before extraction: object 16 MiB + 16 bytes, manifest 8 MiB, upload-session JSON 4 KiB, chunk 1 MiB.
- Successful write operations are recorded in a hash-linked audit ledger under the vault root. Startup verifies the chain and fails closed if it has been altered.
- Operational logs include device and opaque object identifiers, byte counts, and action names. They never include bearer tokens or payload bytes.

## Configuration

Required:
- `LIFEBOAT_CREDENTIALS_FILE`: version-1 JSON credential file; defaults to `/etc/lifeboat/credentials.json`.
- On Unix, the credentials file must be mode `0600` or stricter. The container runs as UID 10001; mount a file readable by that UID.
- Example: [config/credentials.example.json](../config/credentials.example.json). Its all-zero digest is a placeholder and must never be used as a real token.

Optional:
- `LIFEBOAT_VAULT_ROOT`: storage directory. Defaults to `./lifeboat-vault`.
- `LIFEBOAT_BIND`: socket address. Defaults to `127.0.0.1:8080`. For Docker Compose, the container explicitly binds to its private network while the host port is published only on loopback.

### Create a token

Generate at least 32 random bytes. The helper reads the raw token from stdin and prints only its BLAKE3 digest:

```sh
TOKEN="$(openssl rand -hex 32)"
printf '%s' "$TOKEN" | docker run --rm -i --entrypoint /usr/local/bin/lifeboat-vault lifeboat-vault:local credential-hash 00112233445566778899aabbccddeeff
```

Place the returned digest in `active_token_blake3`; retain the raw token only in the owner-authorized client's secret storage. Do not place raw tokens in Git, environment dumps, URLs, shell history, screenshots, or support logs.

### Rotation and revocation

For bounded rotation, set the new digest as `active_token_blake3`, move the old active digest to `previous_token_blake3`, and set `previous_valid_until_unix` to a short UTC Unix-time deadline (maximum recommended overlap: 10 minutes). Deliver the new raw token securely to the authorized client and restart the service. After expiry, remove the previous digest and expiry and restart again.

For immediate revocation, remove the device entry (or remove the previous digest) and restart the service. This phase does not support online hot reload; revocation becomes effective when the service restarts. Previous digest and expiry must be supplied together. Unsupported versions, duplicate IDs, malformed hashes, and insecure Unix file permissions fail startup.

### Compose setup

1. Create the private secrets directory: `mkdir -p secrets`.
2. Copy the example to `secrets/credentials.json` and replace the placeholder digest.
3. Set ownership to UID 10001 and mode 0600: `sudo chown 10001:10001 secrets/credentials.json && chmod 600 secrets/credentials.json`.
4. Start with `docker compose up --build -d`.
5. The host publishes only `127.0.0.1:8080`. Put a TLS-terminating reverse proxy in front for remote access; do not publish raw HTTP to the public Internet.

## Routes

All routes except health require `Authorization: Bearer <device-token>`.

| Method | Path | Purpose |
|---|---|---|
| GET | `/healthz` | Process health only; reveals no vault data |
| PUT | `/v1/devices/{device_id}/objects/{object_id}` | Store immutable ciphertext |
| GET | `/v1/devices/{device_id}/objects/{object_id}` | Retrieve ciphertext after digest verification |
| HEAD | `/v1/devices/{device_id}/objects/{object_id}` | Check object existence, byte count and verified digest |
| PUT | `/v1/devices/{device_id}/manifests/{snapshot_id}` | Store immutable encrypted manifest |
| GET | `/v1/devices/{device_id}/manifests/{snapshot_id}` | Retrieve encrypted manifest after digest verification |
| POST | `/v1/devices/{device_id}/uploads` | Create a persistent fixed-chunk upload session |
| GET | `/v1/devices/{device_id}/uploads/{upload_id}` | Read resumable session progress |
| PUT | `/v1/devices/{device_id}/uploads/{upload_id}/chunks/{index}` | Upload one independently hashed chunk |
| POST | `/v1/devices/{device_id}/uploads/{upload_id}/complete` | Verify full digest and commit immutable object |

For GET, supply `x-lifeboat-content-blake3: <64 lowercase hex characters>`. PUT returns this header with the stored BLAKE3 digest. The API does not interpret object IDs, snapshot contents, file names, or data classes.

## Resumable upload protocol

Create a session with JSON containing `object_id` (64 lowercase hex characters), `total_size`, `chunk_size` (64 KiB–1 MiB), and `expected_blake3` (the expected digest of the complete ciphertext object). The server allocates a persistent sparse part file and returns an opaque 128-bit `upload_id`. The status response includes `received_bytes`, `chunk_count`, and `completed_chunks`.

Upload chunks with zero-based chunk indexes and `x-lifeboat-chunk-blake3`. Every chunk must have the exact expected length and digest. Repeating a successfully acknowledged chunk with the same digest is idempotent; conflicting retries are rejected. The server syncs chunk bytes before persisting the chunk receipt. Completion requires every chunk receipt and a full-object BLAKE3 match before immutable object commit. A completion receipt makes completion retry-safe if the response is lost. Incomplete sessions expire after seven days and at most 32 active sessions are allowed per device.

## Audit ledger

Successful object, manifest, and resumable-upload write transitions are appended to `<vault-root>/audit/events.jsonl`. The chain records sequence, time, device, action, opaque resource identifier, outcome, previous hash and record hash. It does not store credentials or plaintext. The ledger is verified at startup and fails closed if the chain is inconsistent. It is tamper-evident, not tamper-proof: a privileged attacker who can rewrite the whole ledger can recompute it. Production should export/anchor signed checkpoints to a separate administrative domain.

## Operational limitations

The service is single-process. Rate limits, storage serialization and audit locking are process-local. There is no online enrollment/hot reload, multi-node coordination, built-in TLS, audit checkpoint anchoring, committed-object retention/deletion policy, S3 backend, network fuzzing, or independent security review. The local immutable store relies on filesystem hard-link create-if-absent behavior; validate the actual deployment filesystem. Back up the vault volume and audit ledger independently.

The API is a transport layer only. Client-side encryption, authenticated manifest semantics, backup completeness, and restore verification remain separate responsibilities. No production Android client, desktop console, cloud connectors, or complete end-to-end recovery workflow is delivered.

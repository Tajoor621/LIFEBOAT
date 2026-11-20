use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Json, Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use subtle::ConstantTimeEq;

mod audit;

use audit::AuditLedger;
use lifeboat_vault_store::{StoreError, VaultStore};

const MAX_OBJECT_BYTES: usize = 16 * 1024 * 1024 + 16;
const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_STORED_BYTES: u64 = MAX_OBJECT_BYTES as u64;
const MAX_DEVICE_STORAGE_BYTES: u64 = 50 * 1024 * 1024 * 1024;
const RATE_WINDOW: Duration = Duration::from_secs(60);
const MAX_REQUESTS_PER_WINDOW: u32 = 120;

#[derive(Clone)]
struct DeviceCredential {
    active_hash: [u8; 32],
    previous_hash: Option<[u8; 32]>,
    previous_valid_until_unix: Option<u64>,
}

#[derive(serde::Deserialize)]
struct CredentialFile {
    version: u32,
    devices: Vec<CredentialEntry>,
}

#[derive(serde::Deserialize)]
struct CredentialEntry {
    device_id: String,
    active_token_blake3: String,
    previous_token_blake3: Option<String>,
    previous_valid_until_unix: Option<u64>,
}

#[derive(Default)]
struct RateWindow {
    started: Option<Instant>,
    requests: u32,
}

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    // Credential digests only; raw bearer tokens are never stored.
    device_credentials: Arc<HashMap<String, DeviceCredential>>,
    rate_limits: Arc<tokio::sync::Mutex<HashMap<String, RateWindow>>>,
    storage_lock: Arc<tokio::sync::Mutex<()>>,
    audit: Arc<std::sync::Mutex<AuditLedger>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("credential-hash") {
        let device_id = args
            .get(2)
            .ok_or("usage: lifeboat-vault credential-hash <device-id>; provide token on stdin")?;
        if !valid_hex_id(device_id, 32) {
            return Err("device ID must be 32 lowercase hex characters".into());
        }
        let mut token = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut token)?;
        let token = token.trim_end_matches(['\r', '\n']);
        if token.len() < 32 {
            return Err("token must contain at least 32 bytes".into());
        }
        println!("{}", encode_hex(blake3::hash(token.as_bytes()).as_bytes()));
        return Ok(());
    }
    let credentials_path = env::var("LIFEBOAT_CREDENTIALS_FILE")
        .unwrap_or_else(|_| "/etc/lifeboat/credentials.json".to_owned());
    let device_credentials = load_credentials_file(&credentials_path)?;
    if device_credentials.is_empty() {
        return Err("at least one device credential must be configured".into());
    }

    let root = PathBuf::from(
        env::var("LIFEBOAT_VAULT_ROOT").unwrap_or_else(|_| "./lifeboat-vault".to_owned()),
    );
    std::fs::create_dir_all(root.join("devices"))?;
    let audit = AuditLedger::open(root.join("audit"))?;

    let bind: SocketAddr = env::var("LIFEBOAT_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse()?;

    let state = AppState {
        root,
        device_credentials: Arc::new(device_credentials),
        rate_limits: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        storage_lock: Arc::new(tokio::sync::Mutex::new(())),
        audit: Arc::new(std::sync::Mutex::new(audit)),
    };

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "LIFEBOAT vault API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .merge(
            Router::new()
                .route(
                    "/v1/devices/{device_id}/objects/{object_id}",
                    put(put_object).get(get_object).head(head_object),
                )
                .layer(DefaultBodyLimit::max(MAX_OBJECT_BYTES)),
        )
        .merge(
            Router::new()
                .route(
                    "/v1/devices/{device_id}/manifests/{snapshot_id}",
                    put(put_manifest).get(get_manifest),
                )
                .layer(DefaultBodyLimit::max(MAX_MANIFEST_BYTES)),
        )
        .merge(
            Router::new()
                .route("/v1/devices/{device_id}/uploads", post(create_upload))
                .layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/v1/devices/{device_id}/uploads/{upload_id}",
            get(get_upload_status),
        )
        .merge(
            Router::new()
                .route(
                    "/v1/devices/{device_id}/uploads/{upload_id}/chunks/{chunk_index}",
                    put(put_upload_chunk),
                )
                .layer(DefaultBodyLimit::max(1024 * 1024)),
        )
        .route(
            "/v1/devices/{device_id}/uploads/{upload_id}/complete",
            post(complete_upload),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate_request,
        ))
        .with_state(state)
}

async fn authenticate_request(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if path == "/healthz" {
        return next.run(request).await;
    }

    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if segments.len() < 4 || segments[0] != "v1" || segments[1] != "devices" {
        return StatusCode::NOT_FOUND.into_response();
    }
    let device_id = segments[2];
    let valid_route = match segments.get(3).copied() {
        Some("objects" | "manifests") => segments.len() == 5,
        Some("uploads") => matches!(segments.len(), 4..=7),
        _ => false,
    };
    if !valid_hex_id(device_id, 32) || !valid_route {
        return StatusCode::NOT_FOUND.into_response();
    }

    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let Some(token) = token else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(expected) = state.device_credentials.get(device_id) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !credential_matches(expected, token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let mut limits = state.rate_limits.lock().await;
    let window = limits.entry(device_id.to_owned()).or_default();
    let now = Instant::now();
    if window
        .started
        .is_none_or(|started| now.duration_since(started) >= RATE_WINDOW)
    {
        window.started = Some(now);
        window.requests = 0;
    }
    if window.requests >= MAX_REQUESTS_PER_WINDOW {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    window.requests += 1;
    drop(limits);

    next.run(request).await
}

async fn health() -> &'static str {
    "ok"
}

fn load_credentials_file(
    path: &str,
) -> Result<HashMap<String, DeviceCredential>, Box<dyn std::error::Error>> {
    let metadata = std::fs::metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("credentials file permissions must be 0600 or stricter".into());
        }
    }
    let raw: CredentialFile = serde_json::from_slice(&std::fs::read(path)?)?;
    if raw.version != 1 {
        return Err("unsupported credentials file version".into());
    }
    let mut credentials = HashMap::new();
    for entry in raw.devices {
        if !valid_hex_id(&entry.device_id, 32) {
            return Err("invalid device ID in credentials file".into());
        }
        let active_hash =
            decode_hex_32(&entry.active_token_blake3).ok_or("invalid active token digest")?;
        let previous_hash = match entry.previous_token_blake3.as_deref() {
            Some(value) => Some(decode_hex_32(value).ok_or("invalid previous token digest")?),
            None => None,
        };
        if previous_hash.is_some() != entry.previous_valid_until_unix.is_some() {
            return Err("previous token digest and expiry must be supplied together".into());
        }
        let credential = DeviceCredential {
            active_hash,
            previous_hash,
            previous_valid_until_unix: entry.previous_valid_until_unix,
        };
        if credentials.insert(entry.device_id, credential).is_some() {
            return Err("duplicate device ID in credentials file".into());
        }
    }
    Ok(credentials)
}

fn credential_matches(credential: &DeviceCredential, token: &str) -> bool {
    let actual = blake3::hash(token.as_bytes());
    let active = credential.active_hash.ct_eq(actual.as_bytes()).unwrap_u8() == 1;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let previous = credential.previous_hash.as_ref().is_some_and(|hash| {
        credential
            .previous_valid_until_unix
            .is_some_and(|expiry| now <= expiry)
            && hash.ct_eq(actual.as_bytes()).unwrap_u8() == 1
    });
    active || previous
}

#[cfg(test)]
fn parse_device_tokens(input: &str) -> Result<HashMap<String, [u8; 32]>, &'static str> {
    let mut credentials = HashMap::new();
    for entry in input.split(';').filter(|entry| !entry.is_empty()) {
        let (device_id, token) = entry
            .split_once(':')
            .ok_or("credential entries must use device_id:token format")?;
        if !valid_hex_id(device_id, 32) || token.len() < 32 {
            return Err(
                "device IDs must be 32 lowercase hex characters and tokens at least 32 bytes",
            );
        }
        let digest = *blake3::hash(token.as_bytes()).as_bytes();
        if credentials.insert(device_id.to_owned(), digest).is_some() {
            return Err("duplicate device ID in LIFEBOAT_DEVICE_TOKENS");
        }
    }
    Ok(credentials)
}

fn authorized_store(
    state: &AppState,
    device_id: &str,
    headers: &HeaderMap,
) -> Result<VaultStore, StatusCode> {
    if !valid_hex_id(device_id, 32) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let expected = state
        .device_credentials
        .get(device_id)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !credential_matches(expected, token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    VaultStore::open(state.root.join("devices").join(device_id), MAX_STORED_BYTES)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn put_object(
    State(state): State<AppState>,
    Path((device_id, object_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    if !valid_hex_id(&object_id, 64) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if body.len() > MAX_OBJECT_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let _storage_guard = state.storage_lock.lock().await;
    let store = authorized_store(&state, &device_id, &headers)?;
    let current_usage = store.total_bytes().map_err(map_store_error)?;
    if current_usage.saturating_add(body.len() as u64) > MAX_DEVICE_STORAGE_BYTES {
        return Err(StatusCode::INSUFFICIENT_STORAGE);
    }
    let digest = store
        .put_object(&object_id, &body)
        .map_err(map_store_error)?;
    record_audit(&state, &device_id, "object_put", Some(&object_id))?;
    tracing::info!(
        %device_id,
        %object_id,
        bytes = body.len(),
        action = "object_put"
    );
    Ok(digest_response(StatusCode::CREATED, digest))
}

async fn head_object(
    State(state): State<AppState>,
    Path((device_id, object_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if !valid_hex_id(&object_id, 64) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let store = authorized_store(&state, &device_id, &headers)?;
    let (size, digest) = store.object_metadata(&object_id).map_err(map_store_error)?;
    let mut response = digest_response(StatusCode::OK, digest);
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&size.to_string()).expect("decimal length is a valid header"),
    );
    Ok(response)
}

async fn get_object(
    State(state): State<AppState>,
    Path((device_id, object_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if !valid_hex_id(&object_id, 64) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let digest = required_digest(&headers)?;
    let store = authorized_store(&state, &device_id, &headers)?;
    let bytes = store
        .get_object(&object_id, digest)
        .map_err(map_store_error)?;
    tracing::info!(
        %device_id,
        %object_id,
        bytes = bytes.len(),
        action = "object_get"
    );
    Ok(bytes_response(bytes, digest))
}

async fn put_manifest(
    State(state): State<AppState>,
    Path((device_id, snapshot_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    if !valid_hex_id(&snapshot_id, 32) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if body.len() > MAX_MANIFEST_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let _storage_guard = state.storage_lock.lock().await;
    let store = authorized_store(&state, &device_id, &headers)?;
    let current_usage = store.total_bytes().map_err(map_store_error)?;
    if current_usage.saturating_add(body.len() as u64) > MAX_DEVICE_STORAGE_BYTES {
        return Err(StatusCode::INSUFFICIENT_STORAGE);
    }
    let digest = store
        .put_manifest(&snapshot_id, &body)
        .map_err(map_store_error)?;
    record_audit(&state, &device_id, "manifest_put", Some(&snapshot_id))?;
    tracing::info!(
        %device_id,
        %snapshot_id,
        bytes = body.len(),
        action = "manifest_put"
    );
    Ok(digest_response(StatusCode::CREATED, digest))
}

async fn get_manifest(
    State(state): State<AppState>,
    Path((device_id, snapshot_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if !valid_hex_id(&snapshot_id, 32) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let digest = required_digest(&headers)?;
    let store = authorized_store(&state, &device_id, &headers)?;
    let bytes = store
        .get_manifest(&snapshot_id, digest)
        .map_err(map_store_error)?;
    tracing::info!(
        %device_id,
        %snapshot_id,
        bytes = bytes.len(),
        action = "manifest_get"
    );
    Ok(bytes_response(bytes, digest))
}

#[derive(serde::Deserialize)]
struct CreateUploadRequest {
    object_id: String,
    total_size: u64,
    chunk_size: u32,
    expected_blake3: String,
}

async fn create_upload(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CreateUploadRequest>,
) -> Result<Response, StatusCode> {
    if !valid_hex_id(&request.object_id, 64) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let expected = decode_hex_32(&request.expected_blake3).ok_or(StatusCode::BAD_REQUEST)?;
    let _storage_guard = state.storage_lock.lock().await;
    let store = authorized_store(&state, &device_id, &headers)?;
    if store
        .total_bytes()
        .map_err(map_store_error)?
        .saturating_add(request.total_size)
        > MAX_DEVICE_STORAGE_BYTES
    {
        return Err(StatusCode::INSUFFICIENT_STORAGE);
    }
    let info = store
        .create_upload_session(
            &request.object_id,
            request.total_size,
            request.chunk_size,
            expected,
        )
        .map_err(map_store_error)?;
    record_audit(
        &state,
        &device_id,
        "upload_session_create",
        Some(&info.upload_id),
    )?;
    tracing::info!(
        %device_id,
        upload_id = %info.upload_id,
        bytes = info.total_size,
        action = "upload_session_create"
    );
    Ok((StatusCode::CREATED, Json(info)).into_response())
}

async fn get_upload_status(
    State(state): State<AppState>,
    Path((device_id, upload_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let store = authorized_store(&state, &device_id, &headers)?;
    let info = store
        .upload_session_info(&upload_id)
        .map_err(map_store_error)?;
    Ok(Json(info).into_response())
}

async fn put_upload_chunk(
    State(state): State<AppState>,
    Path((device_id, upload_id, chunk_index)): Path<(String, String, usize)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    if body.len() > 1024 * 1024 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let digest_header = headers
        .get("x-lifeboat-chunk-blake3")
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let digest = decode_hex_32(digest_header).ok_or(StatusCode::BAD_REQUEST)?;
    let _storage_guard = state.storage_lock.lock().await;
    let store = authorized_store(&state, &device_id, &headers)?;
    let info = store
        .put_upload_chunk(&upload_id, chunk_index, &body, digest)
        .map_err(map_store_error)?;
    record_audit(&state, &device_id, "upload_chunk_put", Some(&upload_id))?;
    tracing::info!(
        %device_id,
        %upload_id,
        chunk_index,
        bytes = body.len(),
        action = "upload_chunk_put"
    );
    Ok(Json(info).into_response())
}

async fn complete_upload(
    State(state): State<AppState>,
    Path((device_id, upload_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let _storage_guard = state.storage_lock.lock().await;
    let store = authorized_store(&state, &device_id, &headers)?;
    let digest = store.complete_upload(&upload_id).map_err(map_store_error)?;
    record_audit(&state, &device_id, "upload_complete", Some(&upload_id))?;
    tracing::info!(%device_id, %upload_id, action = "upload_complete");
    Ok(digest_response(StatusCode::CREATED, digest))
}

fn record_audit(
    state: &AppState,
    device_id: &str,
    action: &str,
    resource_id: Option<&str>,
) -> Result<(), StatusCode> {
    state
        .audit
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .append(device_id, action, resource_id, "success")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn required_digest(headers: &HeaderMap) -> Result<[u8; 32], StatusCode> {
    let value = headers
        .get("x-lifeboat-content-blake3")
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    decode_hex_32(value).ok_or(StatusCode::BAD_REQUEST)
}

fn digest_response(status: StatusCode, digest: [u8; 32]) -> Response {
    let mut response = status.into_response();
    response.headers_mut().insert(
        "x-lifeboat-content-blake3",
        HeaderValue::from_str(&encode_hex(&digest)).expect("hex digest is a valid header"),
    );
    response
}

fn bytes_response(bytes: Vec<u8>, digest: [u8; 32]) -> Response {
    let mut response = bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        "x-lifeboat-content-blake3",
        HeaderValue::from_str(&encode_hex(&digest)).expect("hex digest is a valid header"),
    );
    response
}

fn map_store_error(error: StoreError) -> StatusCode {
    match error {
        StoreError::InvalidId => StatusCode::BAD_REQUEST,
        StoreError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        StoreError::AlreadyExists | StoreError::SessionConflict => StatusCode::CONFLICT,
        StoreError::SessionNotFound => StatusCode::NOT_FOUND,
        StoreError::Incomplete | StoreError::DigestMismatch => StatusCode::UNPROCESSABLE_ENTITY,
        StoreError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
            StatusCode::NOT_FOUND
        }
        StoreError::Integrity => StatusCode::UNPROCESSABLE_ENTITY,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn valid_hex_id(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if !valid_hex_id(value, 64) {
        return None;
    }
    let mut result = [0u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = (hex_nibble(value.as_bytes()[offset])? << 4)
            | hex_nibble(value.as_bytes()[offset + 1])?;
    }
    Some(result)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn encode_hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use std::sync::atomic::{AtomicU64, Ordering};
    use tower::ServiceExt;

    static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

    fn test_state() -> AppState {
        let token = "a".repeat(40);
        let root = std::env::temp_dir().join(format!(
            "lifeboat-api-test-{}-{}",
            std::process::id(),
            NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        let audit = AuditLedger::open(root.join("audit")).unwrap();
        AppState {
            root,
            device_credentials: Arc::new(HashMap::from([(
                "00112233445566778899aabbccddeeff".to_owned(),
                DeviceCredential {
                    active_hash: *blake3::hash(token.as_bytes()).as_bytes(),
                    previous_hash: None,
                    previous_valid_until_unix: None,
                },
            )])),
            rate_limits: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            storage_lock: Arc::new(tokio::sync::Mutex::new(())),
            audit: Arc::new(std::sync::Mutex::new(audit)),
        }
    }

    #[test]
    fn token_configuration_rejects_short_tokens_and_duplicate_devices() {
        assert!(parse_device_tokens("00112233445566778899aabbccddeeff:short").is_err());
        let duplicate = format!(
            "00112233445566778899aabbccddeeff:{};00112233445566778899aabbccddeeff:{}",
            "a".repeat(40),
            "b".repeat(40)
        );
        assert!(parse_device_tokens(&duplicate).is_err());
    }

    #[test]
    fn token_configuration_accepts_multiple_distinct_devices() {
        let config = format!(
            "00112233445566778899aabbccddeeff:{};ffeeddccbbaa99887766554433221100:{}",
            "a".repeat(40),
            "b".repeat(40)
        );
        assert_eq!(parse_device_tokens(&config).unwrap().len(), 2);
    }

    #[tokio::test]
    async fn health_is_public_but_data_routes_require_authentication() {
        let app = build_router(test_state());

        let health_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health_response.status(), StatusCode::OK);

        let protected_response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(concat!(
                        "/v1/devices/00112233445566778899aabbccddeeff/objects/",
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    ))
                    .body(Body::from("ciphertext"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(protected_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn authenticated_resumable_upload_round_trip_and_device_isolation() {
        let app = build_router(test_state());
        let device_id = "00112233445566778899aabbccddeeff";
        let token = "a".repeat(40);
        let payload = vec![9u8; 65_536];
        let digest = encode_hex(blake3::hash(&payload).as_bytes());
        let create_body = serde_json::json!({
            "object_id": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "total_size": payload.len(),
            "chunk_size": 65_536,
            "expected_blake3": digest.clone()
        })
        .to_string();

        let unauthenticated_large_body = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!(
                        "/v1/devices/{device_id}/objects/{}",
                        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    ))
                    .body(Body::from(vec![0u8; MAX_OBJECT_BYTES + 1]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            unauthenticated_large_body.status(),
            StatusCode::UNAUTHORIZED
        );

        let created = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/devices/{device_id}/uploads"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(create_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(created.into_body(), 4096)
            .await
            .unwrap();
        let info: lifeboat_vault_store::UploadSessionInfo = serde_json::from_slice(&body).unwrap();

        let chunk_digest = encode_hex(blake3::hash(&payload).as_bytes());
        let uploaded = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!(
                        "/v1/devices/{device_id}/uploads/{}/chunks/0",
                        info.upload_id
                    ))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header("x-lifeboat-chunk-blake3", chunk_digest)
                    .body(Body::from(payload.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(uploaded.status(), StatusCode::OK);

        let completed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/devices/{device_id}/uploads/{}/complete",
                        info.upload_id
                    ))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(completed.status(), StatusCode::CREATED);

        let head_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("HEAD")
                    .uri(format!(
                        "/v1/devices/{device_id}/objects/{}",
                        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    ))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(head_response.status(), StatusCode::OK);
        assert_eq!(
            head_response
                .headers()
                .get("x-lifeboat-content-blake3")
                .unwrap()
                .to_str()
                .unwrap(),
            digest.as_str()
        );

        let duplicate_object = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!(
                        "/v1/devices/{device_id}/objects/{}",
                        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    ))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(payload.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(duplicate_object.status(), StatusCode::CONFLICT);

        let cross_device = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/devices/ffeeddccbbaa99887766554433221100/uploads/{}",
                        info.upload_id
                    ))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cross_device.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn per_device_rate_limit_rejects_request_121_in_window() {
        let state = test_state();
        let root = state.root.clone();
        let app = build_router(state);
        let device_id = "00112233445566778899aabbccddeeff";
        let token = "a".repeat(40);
        let object_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let digest = "00".repeat(32);

        for _ in 0..MAX_REQUESTS_PER_WINDOW {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(format!("/v1/devices/{device_id}/objects/{object_id}"))
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .header("x-lifeboat-content-blake3", digest.as_str())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        let limited = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/devices/{device_id}/objects/{object_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header("x-lifeboat-content-blake3", digest)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn previous_token_is_accepted_only_during_rotation_window() {
        let active = "a".repeat(40);
        let previous = "b".repeat(40);
        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 60;
        let credential = DeviceCredential {
            active_hash: *blake3::hash(active.as_bytes()).as_bytes(),
            previous_hash: Some(*blake3::hash(previous.as_bytes()).as_bytes()),
            previous_valid_until_unix: Some(expiry),
        };
        assert!(credential_matches(&credential, &active));
        assert!(credential_matches(&credential, &previous));
        let expired = DeviceCredential {
            previous_valid_until_unix: Some(1),
            ..credential
        };
        assert!(!credential_matches(&expired, &previous));
    }

    #[test]
    fn versioned_credentials_file_loads_active_and_previous_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        let active = "a".repeat(40);
        let previous = "b".repeat(40);
        let active_hash = encode_hex(blake3::hash(active.as_bytes()).as_bytes());
        let previous_hash = encode_hex(blake3::hash(previous.as_bytes()).as_bytes());
        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 60;
        let json = format!(
            r#"{{"version":1,"devices":[{{"device_id":"00112233445566778899aabbccddeeff","active_token_blake3":"{active_hash}","previous_token_blake3":"{previous_hash}","previous_valid_until_unix":{expiry}}}]}}"#
        );
        std::fs::write(&path, json).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let loaded = load_credentials_file(path.to_str().unwrap()).unwrap();
        let credential = loaded.get("00112233445566778899aabbccddeeff").unwrap();
        assert!(credential_matches(credential, &active));
        assert!(credential_matches(credential, &previous));
    }

    #[tokio::test]
    async fn manifest_body_limit_is_enforced_by_route_extractor() {
        let state = test_state();
        let app = build_router(state);
        let device_id = "00112233445566778899aabbccddeeff";
        let token = "a".repeat(40);
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!(
                        "/v1/devices/{device_id}/manifests/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    ))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(vec![0u8; MAX_MANIFEST_BYTES + 1]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[test]
    fn digest_parser_accepts_only_canonical_lowercase_hex() {
        assert_eq!(decode_hex_32(&"ab".repeat(32)), Some([0xab; 32]));
        assert!(decode_hex_32(&"AB".repeat(32)).is_none());
        assert!(decode_hex_32("not-a-digest").is_none());
    }
}

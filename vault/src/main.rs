use std::{collections::HashMap, env, net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, put},
    Router,
};
use subtle::ConstantTimeEq;

use lifeboat_vault_store::{StoreError, VaultStore};

const MAX_OBJECT_BYTES: usize = 16 * 1024 * 1024 + 16;
const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_STORED_BYTES: u64 = MAX_OBJECT_BYTES as u64;

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    // Only keyed hashes are retained in memory; raw bearer tokens are never logged or stored.
    device_token_hashes: Arc<HashMap<String, [u8; 32]>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let token_config = env::var("LIFEBOAT_DEVICE_TOKENS")
        .map_err(|_| "LIFEBOAT_DEVICE_TOKENS is required; refusing to start without device credentials")?;
    let device_token_hashes = parse_device_tokens(&token_config)?;
    if device_token_hashes.is_empty() {
        return Err("at least one device credential must be configured".into());
    }

    let root = PathBuf::from(
        env::var("LIFEBOAT_VAULT_ROOT").unwrap_or_else(|_| "./lifeboat-vault".to_owned()),
    );
    std::fs::create_dir_all(root.join("devices"))?;

    let bind: SocketAddr = env::var("LIFEBOAT_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse()?;

    let state = AppState {
        root,
        device_token_hashes: Arc::new(device_token_hashes),
    };

    let app = Router::new()
        .route("/healthz", get(health))
        .route(
            "/v1/devices/{device_id}/objects/{object_id}",
            put(put_object).get(get_object),
        )
        .route(
            "/v1/devices/{device_id}/manifests/{snapshot_id}",
            put(put_manifest).get(get_manifest),
        )
        .layer(DefaultBodyLimit::max(MAX_OBJECT_BYTES))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "LIFEBOAT vault API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

fn parse_device_tokens(input: &str) -> Result<HashMap<String, [u8; 32]>, &'static str> {
    let mut credentials = HashMap::new();
    for entry in input.split(';').filter(|entry| !entry.is_empty()) {
        let (device_id, token) = entry
            .split_once(':')
            .ok_or("credential entries must use device_id:token format")?;
        if !valid_hex_id(device_id, 32) || token.len() < 32 {
            return Err("device IDs must be 32 lowercase hex characters and tokens at least 32 bytes");
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
        .device_token_hashes
        .get(device_id)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let actual = blake3::hash(token.as_bytes());
    if expected.ct_eq(actual.as_bytes()).unwrap_u8() != 1 {
        return Err(StatusCode::UNAUTHORIZED);
    }

    VaultStore::open(
        state.root.join("devices").join(device_id),
        MAX_STORED_BYTES,
    )
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
    let store = authorized_store(&state, &device_id, &headers)?;
    let digest = store
        .put_object(&object_id, &body)
        .map_err(map_store_error)?;
    tracing::info!(%device_id, %object_id, bytes = body.len(), action = "object_put");
    Ok(digest_response(StatusCode::CREATED, digest))
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
    tracing::info!(%device_id, %object_id, bytes = bytes.len(), action = "object_get");
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
    let store = authorized_store(&state, &device_id, &headers)?;
    let digest = store
        .put_manifest(&snapshot_id, &body)
        .map_err(map_store_error)?;
    tracing::info!(%device_id, %snapshot_id, bytes = body.len(), action = "manifest_put");
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
    tracing::info!(%device_id, %snapshot_id, bytes = bytes.len(), action = "manifest_get");
    Ok(bytes_response(bytes, digest))
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
        StoreError::AlreadyExists => StatusCode::CONFLICT,
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
    use tower::ServiceExt;

    fn test_state() -> AppState {
        let token = "a".repeat(40);
        AppState {
            root: std::env::temp_dir().join(format!("lifeboat-api-test-{}", std::process::id())),
            device_token_hashes: Arc::new(HashMap::from([(
                "00112233445566778899aabbccddeeff".to_owned(),
                *blake3::hash(token.as_bytes()).as_bytes(),
            )])),
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
        let app = Router::new()
            .route("/healthz", get(health))
            .route(
                "/v1/devices/{device_id}/objects/{object_id}",
                put(put_object).get(get_object),
            )
            .route(
                "/v1/devices/{device_id}/manifests/{snapshot_id}",
                put(put_manifest).get(get_manifest),
            )
            .layer(DefaultBodyLimit::max(MAX_OBJECT_BYTES))
            .with_state(test_state());

        let health_response = app
            .clone()
            .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(health_response.status(), StatusCode::OK);

        let protected_response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/v1/devices/00112233445566778899aabbccddeeff/objects/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                    .body(Body::from("ciphertext"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(protected_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn digest_parser_accepts_only_canonical_lowercase_hex() {
        assert_eq!(decode_hex_32(&"ab".repeat(32)), Some([0xab; 32]));
        assert!(decode_hex_32(&"AB".repeat(32)).is_none());
        assert!(decode_hex_32("not-a-digest").is_none());
    }
}

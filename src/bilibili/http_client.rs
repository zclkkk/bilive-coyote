use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Uri};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

pub(super) type HttpBody = Full<Bytes>;

type InnerClient = Client<HttpsConnector<HttpConnector>, HttpBody>;

#[derive(Clone)]
pub(super) struct HyperHttpClient {
    inner: InnerClient,
}

impl HyperHttpClient {
    pub(super) fn new() -> Self {
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_provider_and_webpki_roots(rustls::crypto::ring::default_provider())
            .expect("ring provider supports rustls default protocol versions")
            .https_only()
            .enable_http1()
            .build();
        let inner = Client::builder(TokioExecutor::new()).build(https);
        Self { inner }
    }

    pub(super) async fn send(&self, req: Request<HttpBody>) -> Result<Bytes, HttpError> {
        let resp = self.inner.request(req).await?;
        Ok(resp.into_body().collect().await?.to_bytes())
    }
}

pub(super) fn uri(raw: &str) -> Result<Uri, HttpError> {
    raw.parse::<Uri>().map_err(|source| HttpError::InvalidUri {
        uri: raw.to_string(),
        source,
    })
}

pub(super) fn empty_body() -> HttpBody {
    Full::new(Bytes::new())
}

pub(super) fn json_body(value: &serde_json::Value) -> Result<HttpBody, HttpError> {
    Ok(Full::new(Bytes::from(serde_json::to_vec(value)?)))
}

#[derive(Debug, thiserror::Error)]
pub(super) enum HttpError {
    #[error("invalid URI {uri}: {source}")]
    InvalidUri {
        uri: String,
        source: hyper::http::uri::InvalidUri,
    },
    #[error("invalid request: {0}")]
    Request(#[from] hyper::http::Error),
    #[error("HTTP request failed: {0}")]
    Client(#[from] hyper_util::client::legacy::Error),
    #[error("read response body failed: {0}")]
    Body(#[from] hyper::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

use std::convert::TryFrom;

use crate::{
    cf::Cf, error::Error, headers::Headers, http::Method, ByteStream, FormData, RequestInit, Result,
};

use serde::de::DeserializeOwned;
use std::borrow::Cow;
use url::{form_urlencoded::Parse, Url};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use worker_sys::{Request as EdgeRequest, RequestInit as EdgeRequestInit};

/// A [Request](https://developer.mozilla.org/en-US/docs/Web/API/Request) representation for
/// handling incoming and creating outbound HTTP requests.
#[derive(Debug)]
pub struct Request {
    method: Method,
    path: String,
    headers: Headers,
    cf: Cf,
    edge_request: EdgeRequest,
    body_used: bool,
    immutable: bool,
}

impl From<EdgeRequest> for Request {
    fn from(req: EdgeRequest) -> Self {
        Self {
            method: req.method().into(),
            path: Url::parse(&req.url())
                .map(|u| u.path().into())
                .unwrap_or_else(|_| {
                    let u = req.url();
                    if !u.starts_with('/') {
                        return "/".to_string() + &u;
                    }
                    u
                }),
            headers: Headers(req.headers()),
            cf: req.cf().into(),
            edge_request: req,
            body_used: false,
            immutable: true,
        }
    }
}

impl TryFrom<Request> for EdgeRequest {
    type Error = Error;
    fn try_from(req: Request) -> Result<Self> {
        req.inner().clone().map_err(Error::from)
    }
}

impl TryFrom<&Request> for EdgeRequest {
    type Error = Error;
    fn try_from(req: &Request) -> Result<Self> {
        req.inner().clone().map_err(Error::from)
    }
}

impl Request {
    /// Construct a new `Request` with an HTTP Method.
    pub fn new(uri: &str, method: Method) -> Result<Self> {
        EdgeRequest::new_with_str_and_init(uri, EdgeRequestInit::new().method(method.as_ref()))
            .map(|req| {
                let mut req: Request = req.into();
                req.immutable = false;
                req
            })
            .map_err(|e| {
                Error::JsError(
                    e.as_string()
                        .unwrap_or_else(|| "invalid URL or method for Request".to_string()),
                )
            })
    }

    /// Construct a new `Request` with a `RequestInit` configuration.
    pub fn new_with_init(uri: &str, init: &RequestInit) -> Result<Self> {
        EdgeRequest::new_with_str_and_init(uri, &init.into())
            .map(|req| {
                let mut req: Request = req.into();
                req.immutable = false;
                req
            })
            .map_err(|e| {
                Error::JsError(
                    e.as_string()
                        .unwrap_or_else(|| "invalid URL or options for Request".to_string()),
                )
            })
    }

    /// Access this request's body encoded as JSON.
    pub async fn json<B: DeserializeOwned>(&mut self) -> Result<B> {
        if !self.body_used {
            self.body_used = true;
            return JsFuture::from(self.edge_request.json()?)
                .await
                .map_err(|e| {
                    Error::JsError(
                        e.as_string()
                            .unwrap_or_else(|| "failed to get JSON for body value".into()),
                    )
                })
                .and_then(|val| serde_wasm_bindgen::from_value(val).map_err(Error::from));
        }

        Err(Error::BodyUsed)
    }

    /// Access this request's body as plaintext.
    pub async fn text(&mut self) -> Result<String> {
        if !self.body_used {
            self.body_used = true;
            return JsFuture::from(self.edge_request.text()?)
                .await
                .map(|val| val.as_string().unwrap())
                .map_err(|e| {
                    Error::JsError(
                        e.as_string()
                            .unwrap_or_else(|| "failed to get text for body value".into()),
                    )
                });
        }

        Err(Error::BodyUsed)
    }

    /// Access this request's body as raw bytes.
    pub async fn bytes(&mut self) -> Result<Vec<u8>> {
        if !self.body_used {
            self.body_used = true;
            return JsFuture::from(self.edge_request.array_buffer()?)
                .await
                .map(|val| js_sys::Uint8Array::new(&val).to_vec())
                .map_err(|e| {
                    Error::JsError(
                        e.as_string()
                            .unwrap_or_else(|| "failed to read array buffer from request".into()),
                    )
                });
        }

        Err(Error::BodyUsed)
    }

    /// Access this request's body as a form-encoded payload and pull out fields and files.
    pub async fn form_data(&mut self) -> Result<FormData> {
        if !self.body_used {
            self.body_used = true;
            return JsFuture::from(self.edge_request.form_data()?)
                .await
                .map(|val| val.into())
                .map_err(|e| {
                    Error::JsError(
                        e.as_string()
                            .unwrap_or_else(|| "failed to get form data from request".into()),
                    )
                });
        }

        Err(Error::BodyUsed)
    }

    /// Access this request's body as a [`Stream`](futures::stream::Stream) of bytes.
    pub fn stream(&mut self) -> Result<ByteStream> {
        if self.body_used {
            return Err(Error::BodyUsed);
        }

        self.body_used = true;

        let stream = self
            .edge_request
            .body()
            .ok_or_else(|| Error::RustError("no body for request".into()))?;

        let stream = wasm_streams::ReadableStream::from_raw(stream.dyn_into().unwrap());
        Ok(ByteStream {
            inner: stream.into_stream(),
        })
    }

    /// Get the `Headers` for this request.
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    /// Get a mutable reference to this request's `Headers`.
    /// **Note:** they can only be modified if the request was created from scratch or cloned.
    pub fn headers_mut(&mut self) -> Result<&mut Headers> {
        if self.immutable {
            return Err(Error::JsError(
                "Cannot get a mutable reference to an immutable headers object.".into(),
            ));
        }
        Ok(&mut self.headers)
    }

    /// Access this request's Cloudflare-specific properties.
    pub fn cf(&self) -> &Cf {
        &self.cf
    }

    /// The HTTP Method associated with this `Request`.
    pub fn method(&self) -> Method {
        self.method.clone()
    }

    /// The URL Path of this `Request`.
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// Get a mutable reference to this request's path.
    /// **Note:** they can only be modified if the request was created from scratch or cloned.
    pub fn path_mut(&mut self) -> Result<&mut String> {
        if self.immutable {
            return Err(Error::JsError(
                "Cannot get a mutable reference to an immutable path.".into(),
            ));
        }
        Ok(&mut self.path)
    }

    /// The parsed [`url::Url`] of this `Request`.
    pub fn url(&self) -> Result<Url> {
        let url = self.edge_request.url();
        url.parse()
            .map_err(|e| Error::RustError(format!("failed to parse Url from {}: {}", e, url)))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn clone(&self) -> Result<Self> {
        self.edge_request
            .clone()
            .map(|req| req.into())
            .map_err(Error::from)
    }

    pub fn inner(&self) -> &EdgeRequest {
        &self.edge_request
    }
}

/// Used to add additional helper functions to url::Url
pub trait UrlExt {
    /// Given a query parameter, returns the value of the first occurrence of that parameter if it
    /// exists
    fn param<'a>(&'a self, key: &'a str) -> Option<Cow<'a, str>> {
        self.param_iter(key).next()
    }
    /// Given a query parameter, returns an Iterator of values for that parameter in the url's
    /// query string
    fn param_iter<'a>(&'a self, key: &'a str) -> ParamIter<'a>;
}

impl UrlExt for Url {
    fn param_iter<'a>(&'a self, key: &'a str) -> ParamIter<'a> {
        ParamIter {
            inner: self.query_pairs(),
            key,
        }
    }
}

pub struct ParamIter<'a> {
    inner: Parse<'a>,
    key: &'a str,
}

impl<'a> Iterator for ParamIter<'a> {
    type Item = Cow<'a, str>;

    fn next(&mut self) -> Option<Self::Item> {
        let key = self.key;
        Some(self.inner.find(|(k, _)| k == key)?.1)
    }
}

#[test]
fn url_param_works() {
    let url = Url::parse("https://example.com/foo.html?a=foo&b=bar&a=baz").unwrap();
    assert_eq!(url.param("a").as_deref(), Some("foo"));
    assert_eq!(url.param("b").as_deref(), Some("bar"));
    assert_eq!(url.param("c").as_deref(), None);
    let mut a_values = url.param_iter("a");
    assert_eq!(a_values.next().as_deref(), Some("foo"));
    assert_eq!(a_values.next().as_deref(), Some("baz"));
    assert_eq!(a_values.next(), None);
}

mod error;
mod network_error;
mod fetch_config;
mod fetch_options;
mod fetch_response;
mod utils;

use anyhow::anyhow;
use bytes::Bytes;
pub use error::{DeserializationError, FetchError, FetchResult, SerializationError};
pub use network_error::NetworkError;
pub use fetch_config::FetchConfig;
pub use fetch_options::{ContentType, FetchOptions};
pub use fetch_response::FetchResponse;
pub use reqwest;
pub use reqwest::StatusCode;
use reqwest::{header::HeaderMap, Client, ClientBuilder, RequestBuilder, Response, Url};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::{collections::HashMap, time::Duration};
use utils::{map_to_reqwest_headers, reqwest_headers_to_map};

pub type FetchHeaders = HashMap<String, String>;
pub const USER_AGENT: &'static str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub struct Fetch {
    client: Client,
    pub config: Option<FetchConfig>,
    base_url: String,
}

impl Default for Fetch {
    fn default() -> Self {
        let mut headers: FetchHeaders = HashMap::new();
        Self::insert_default_headers(&mut headers, Default::default());

        Self {
            client: ClientBuilder::default()
                .default_headers(map_to_reqwest_headers(&headers).unwrap())
                .build()
                .unwrap(),
            config: Some(FetchConfig {
                headers: Some(headers),
                ..Default::default()
            }),
            base_url: Default::default(),
        }
    }
}

impl Fetch {
    /// Creates a new instance of Fetch with a set base url and optional Options
    ///
    /// # Example
    /// ```rust
    /// use rust_fetch::Fetch;
    /// let client = Fetch::new("http://localhost", None);
    /// assert_ne!(true, client.is_err());
    ///
    /// ```
    pub fn new(base_url: &str, options: Option<FetchConfig>) -> FetchResult<Self> {
        let mut options = options.unwrap_or_default();
        let mut headers = options
            .headers
            .as_ref()
            .map(|r| r.clone())
            .unwrap_or_default();

        Self::insert_default_headers(&mut headers, Some(&options));
        options.headers = Some(headers);

        let default_headers: HeaderMap;
        let mut client = ClientBuilder::default();
        if let Some(headers) = &options.headers {
            default_headers = map_to_reqwest_headers(&headers)?;
            client = client.default_headers(default_headers);
        }
        if let Some(timeout) = &options.timeout_ms {
            client = client.timeout(Duration::from_millis(timeout.to_owned()))
        }

        Ok(Self {
            base_url: base_url.to_string(),
            config: Some(options),
            client: client
                .build()
                .map_err(|e| FetchError::Unknown(anyhow!(e)))?,
            ..Default::default()
        })
    }

    fn insert_default_headers(headers: &mut FetchHeaders, config: Option<&FetchConfig>) {
        headers.insert("user-agent".to_string(), USER_AGENT.to_string());
        if let Some(config) = config {
            headers.insert(
                reqwest::header::CONTENT_TYPE.to_string(),
                config.content_type.clone().to_string(),
            );
            headers.insert(
                reqwest::header::ACCEPT.to_string(),
                config.accept.clone().to_string(),
            );
        }
    }

    /// Sets the default headers for this instance of Fetch.
    ///
    /// # Example
    /// ```rust
    /// use rust_fetch::{Fetch, map_string};
    ///
    /// let mut client = Fetch::new("http://localhost", None).unwrap();
    /// let set_header_result = client.set_default_headers(Some(map_string!{ header1 : "header 1 value" }));
    /// assert_ne!(true, set_header_result.is_err());
    ///
    /// ```
    pub fn set_default_headers(&mut self, headers: Option<FetchHeaders>) -> FetchResult<()> {
        let mut headers = headers.unwrap_or_default();

        Self::insert_default_headers(&mut headers, self.config.as_ref());

        let opts: FetchConfig = FetchConfig {
            headers: Some(headers),
            ..self.config.clone().unwrap_or(Default::default())
        };

        let new_fetch = Self::new(&self.base_url, Some(opts))?;
        self.client = new_fetch.client;
        self.config = new_fetch.config;

        Ok(())
    }

    pub fn build_url(&self, endpoint: &str, options: Option<&FetchOptions>) -> FetchResult<Url> {
        let mut built_string = String::new();
        built_string += &self.base_url;

        if built_string.chars().nth(built_string.chars().count() - 1) != Some('/')
            && endpoint.chars().nth(0) != Some('/')
        {
            built_string += "/";
        }

        built_string += endpoint;
        if let Some(options) = options {
            if let Some(params) = &options.params {
                let mut added_param = false;
                for (index, (key, value)) in params.iter().enumerate() {
                    if !added_param {
                        built_string += "?";
                        added_param = true;
                    }
                    built_string += &format!("{key}={value}");
                    if index < params.len() - 1 {
                        built_string += "&";
                    }
                }
            }
        }

        let url: Url = built_string
            .parse()
            .map_err(|_| FetchError::InvalidUrl(built_string))?;

        Ok(url)
    }

    fn make_body<U>(
        &self,
        data: U,
        options: Option<&FetchOptions>,
    ) -> FetchResult<(Vec<u8>, ContentType)>
    where
        U: Serialize,
    {
        let mut content_type: ContentType = Default::default();

        if let Some(opts) = options {
            if let Some(ref c_type) = opts.content_type {
                content_type = c_type.clone();
            } else {
                if let Some(ref config) = self.config {
                    content_type = config.content_type.clone();
                }
            }
        } else {
            if let Some(ref config) = self.config {
                content_type = config.content_type.clone();
            }
        }

        let data_to_return = match content_type {
            ContentType::Json => serde_json::to_vec(&data)
                .map_err(|e| FetchError::SerializationError(SerializationError::Json(e)))?,
            ContentType::TextXml | ContentType::ApplicationXml => serde_xml_rs::to_string(&data)
                .map_err(|e| FetchError::SerializationError(SerializationError::Xml(e)))?
                .into_bytes(),
            ContentType::UrlEncoded => serde_urlencoded::to_string(&data)
                .map_err(|e| FetchError::SerializationError(SerializationError::UrlEncoded(e)))?
                .into_bytes(),
        };

        return Ok((data_to_return, content_type));
    }

    fn build_request<U>(
        &self,
        data: Option<U>,
        options: Option<&FetchOptions>,
        original_builder: RequestBuilder,
    ) -> FetchResult<RequestBuilder>
    where
        U: Serialize,
    {
        let mut builder = original_builder;
        if let Some(options) = options {
            if let Some(headers) = &options.headers {
                builder = builder.headers(map_to_reqwest_headers(headers)?);
            }
        };
        if let Some(body) = data {
            let (body, content_type) = self.make_body(body, options)?;
            builder = builder.body(body);
            builder = builder.header(reqwest::header::CONTENT_TYPE, format!("{content_type}"));
        }
        if let Some(opts) = options {
            if let Some(ref accept) = opts.accept {
                builder = builder.header(reqwest::header::ACCEPT.to_string(), accept.to_string());
            }
        }

        return Ok(builder);
    }

    fn deserialize_response<T>(
        &self,
        raw_body: &Bytes,
        content_type: ContentType,
    ) -> FetchResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        return match content_type {
            ContentType::Json => Ok(serde_json::from_slice::<T>(raw_body)
                .map_err(|e| FetchError::DeserializationError(DeserializationError::Json(e)))?),
            ContentType::TextXml | ContentType::ApplicationXml => {
                let body_string = String::from_utf8(raw_body.to_vec()).map_err(|_| {
                    FetchError::DeserializationError(DeserializationError::Unknown(String::from(
                        "Response body does not contain valid Utf8",
                    )))
                })?;
                Ok(serde_xml_rs::from_str::<T>(&body_string)
                    .map_err(|e| FetchError::DeserializationError(DeserializationError::Xml(e)))?)
            }
            ContentType::UrlEncoded => {
                Ok(serde_urlencoded::from_bytes::<T>(raw_body).map_err(|e| {
                    FetchError::DeserializationError(DeserializationError::UrlEncoded(e))
                })?)
            }
        };
    }

    async fn check_response_and_return_err(&self, response: Response) -> FetchResult<Response> {
        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(FetchError::NetworkError(NetworkError::new(response).await));
        }
        Ok(response)
    }

    async fn response_to_fetch_response<T>(
        &self,
        response: Response,
        deserialize_body: bool,
    ) -> FetchResult<FetchResponse<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response = self.check_response_and_return_err(response).await?;
        let remote_content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .map(|c_type| {
                c_type
                    .to_str()
                    .ok()
                    .map(|s| s.to_owned())
                    .unwrap_or_default()
            })
            .map(|string| ContentType::from_str(&string).ok().unwrap_or_default());

        let headers = response.headers().clone();
        let remote_address = response.remote_addr();
        let status = response.status();

        let raw_body = response.bytes().await.ok();
        let mut body: Option<T> = None;

        if let Some(raw_body) = &raw_body {
            if deserialize_body {
                if let Some(response_content_type) = remote_content_type {
                    body = Some(self.deserialize_response::<T>(raw_body, response_content_type)?);
                } else {
                    body = Some(self.deserialize_response::<T>(raw_body, ContentType::Json)?);
                }
            }
        }

        return Ok(FetchResponse {
            body,
            raw_body,
            status,
            response_headers: reqwest_headers_to_map(&headers)?,
            remote_address,
        });
    }

    /// Sends an HTTP Post request to the configured remote server
    ///
    /// * `endpoint` - The remote endpoint. This gets joined with the base_url configured in the ::new() method
    /// * `data` - Optional data to send to the remote endpoint (to be serialized as JSON). If `None`, then no data is sent instead of `null`
    /// * `options` - The `FetchOptions` for this call. Allows setting of headers and/or query params
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use rust_fetch::Fetch;
    ///
    /// #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    /// struct ToReturn {}
    ///
    /// #[derive(serde::Serialize, serde::Deserialize, Debug)]
    /// struct ToSend {
    ///     test_key: String,
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let server = MockServer::start();
    ///
    ///     server.mock(|when, then| {
    ///         when.path("/test").method(POST);
    ///         then.status(200).json_body(serde_json::json!({}));
    ///     });
    ///
    ///     let fetch = Fetch::new(&server.base_url(), None).unwrap();
    ///
    ///      let response = fetch
    ///         .post::<ToReturn, ToSend>(
    ///             "/test",
    ///             Some(ToSend {
    ///                 test_key: "Testing".to_string(),
    ///             }),
    ///             Some(rust_fetch::FetchOptions {
    ///                 params: Some(rust_fetch::map_string! {param1 : "value1"}),
    ///                 ..Default::default()
    ///             }),
    ///         )
    ///         .await.unwrap();
    ///     assert_eq!(&200, &response.status);
    ///     assert_eq!(ToReturn {}, response.body.unwrap());
    /// }
    /// ```
    pub async fn post<T, U>(
        &self,
        endpoint: &str,
        data: Option<U>,
        options: Option<FetchOptions>,
    ) -> FetchResult<FetchResponse<T>>
    where
        T: for<'de> Deserialize<'de>,
        U: Serialize,
    {
        let options = options.unwrap_or_default();
        let response = self
            .build_request(
                data,
                Some(&options),
                self.client.post(self.build_url(endpoint, Some(&options))?),
            )?
            .send()
            .await
            .map_err(|e| FetchError::UnableToSendRequest { err: e })?;

        return Ok(self
            .response_to_fetch_response(response, options.deserialize_body)
            .await?);
    }

    /// Sends an HTTP GET request to the configured remote server
    ///
    /// * `endpoint` - The remote endpoint. This gets joined with the base_url configured in the ::new() method
    /// * `options` - The `FetchOptions` for this call. Allows setting of headers and/or query params
    ///
    /// # Example
    ///
    /// ```rust
    ///     use rust_fetch::Fetch;
    ///     use httpmock::prelude::*;
    ///
    ///     #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    ///     struct ToReturn {
    ///         
    ///     }
    ///
    ///     #[tokio::main]
    ///     async fn main() {
    ///         let server = MockServer::start();
    ///         
    ///         server.mock(|when, then|{
    ///             when.path("/test");
    ///             then.status(200).json_body(serde_json::json!({}));
    ///         });
    ///
    ///         let fetch = Fetch::new(&server.base_url(), None).unwrap();
    ///
    ///         let response = fetch.get::<ToReturn>("/test", Some(rust_fetch::FetchOptions
    ///         {
    ///             params: Some(rust_fetch::map_string!{param1 : "value1"}),
    ///             ..Default::default()
    ///         })).await.unwrap();
    ///         assert_eq!(&200, &response.status);
    ///         assert_eq!(ToReturn{}, response.body.unwrap());
    ///     }
    ///     
    /// ```
    pub async fn get<T>(
        &self,
        endpoint: &str,
        options: Option<FetchOptions>,
    ) -> FetchResult<FetchResponse<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let options = options.unwrap_or_default();
        let response = self
            .build_request::<()>(
                None,
                Some(&options),
                self.client.get(self.build_url(endpoint, Some(&options))?),
            )?
            .send()
            .await
            .map_err(|e| FetchError::UnableToSendRequest { err: e })?;

        return Ok(self
            .response_to_fetch_response(response, options.deserialize_body)
            .await?);
    }

    /// Sends an HTTP DELETE request to the configured remote server
    ///
    /// * `endpoint` - The remote endpoint. This gets joined with the base_url configured in the ::new() method
    /// * `data` - The optional data to send the the remote endpoint
    /// * `options` - The `FetchOptions` for this call. Allows setting of headers and/or query params
    ///
    /// # Example
    /// ```rust
    ///     use rust_fetch::Fetch;
    ///     use httpmock::prelude::*;
    ///
    ///     #[derive(serde::Deserialize, Debug, PartialEq)]
    ///     struct ToReturn {}
    ///
    ///     #[tokio::main]
    ///     async fn main() {
    ///         let server = MockServer::start();
    ///
    ///         server.mock(| when, then | {
    ///             when.path("/test").method(DELETE);
    ///             then.status(200).json_body(serde_json::json!({}));
    ///         });
    ///
    ///         let client = Fetch::new(&server.base_url(), None).unwrap();
    ///
    ///         let res = client.delete::<(), ToReturn>("/test", None, None).await.unwrap();
    ///         assert_eq!(&200, &res.status);
    ///         assert_eq!(ToReturn {}, res.body.unwrap());
    ///     }
    /// ```
    pub async fn delete<T, U>(
        &self,
        endpoint: &str,
        data: Option<T>,
        options: Option<FetchOptions>,
    ) -> FetchResult<FetchResponse<U>>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let options = options.unwrap_or_default();
        let response = self
            .build_request(
                data,
                Some(&options),
                self.client
                    .delete(self.build_url(endpoint, Some(&options))?),
            )?
            .send()
            .await
            .map_err(|e| FetchError::UnableToSendRequest { err: e })?;

        return Ok(self
            .response_to_fetch_response(response, options.deserialize_body)
            .await?);
    }

    /// Sends an HTTP PUT request to the configured remote server
    ///
    /// * `endpoint` - The remote endpoint. This gets joined with the base_url configured in the ::new() method
    /// * `data` - The optional data to send the the remote endpoint
    /// * `options` - The `FetchOptions` for this call. Allows setting of headers and/or query params
    ///
    /// # Example
    /// ```rust
    ///     use rust_fetch::Fetch;
    ///     use httpmock::prelude::*;
    ///
    ///     #[derive(serde::Deserialize, Debug, PartialEq)]
    ///     struct ToReturn {}
    ///
    ///     #[tokio::main]
    ///     async fn main() {
    ///         let server = MockServer::start();
    ///
    ///         server.mock(| when, then | {
    ///             when.path("/test").method(PUT);
    ///             then.status(200).json_body(serde_json::json!({}));
    ///         });
    ///
    ///         let client = Fetch::new(&server.base_url(), None).unwrap();
    ///
    ///         let res = client.put::<(), ToReturn>("/test", None, None).await.unwrap();
    ///         assert_eq!(&200, &res.status);
    ///         assert_eq!(ToReturn {}, res.body.unwrap());
    ///     }
    /// ```
    pub async fn put<T, U>(
        &self,
        endpoint: &str,
        data: Option<T>,
        options: Option<FetchOptions>,
    ) -> FetchResult<FetchResponse<U>>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let options = options.unwrap_or_default();
        let response = self
            .build_request(
                data,
                Some(&options),
                self.client.put(self.build_url(endpoint, Some(&options))?),
            )?
            .send()
            .await
            .map_err(|e| FetchError::UnableToSendRequest { err: e })?;

        return Ok(self
            .response_to_fetch_response(response, options.deserialize_body)
            .await?);
    }

    /// Sends an HTTP PATCH request to the configured remote server
    ///
    /// * `endpoint` - The remote endpoint. This gets joined with the base_url configured in the ::new() method
    /// * `data` - The optional data to send the the remote endpoint
    /// * `options` - The `FetchOptions` for this call. Allows setting of headers and/or query params
    ///
    /// # Example
    /// ```rust
    ///     use rust_fetch::Fetch;
    ///     use httpmock::prelude::*;
    ///
    ///     #[derive(serde::Deserialize, Debug, PartialEq)]
    ///     struct ToReturn {}
    ///
    ///     #[tokio::main]
    ///     async fn main() {
    ///         let server = MockServer::start();
    ///
    ///         server.mock(| when, then | {
    ///             when.path("/test").method(httpmock::Method::PATCH);
    ///             then.status(200).json_body(serde_json::json!({}));
    ///         });
    ///
    ///         let client = Fetch::new(&server.base_url(), None).unwrap();
    ///
    ///         let res = client.patch::<(), ToReturn>("/test", None, None).await.unwrap();
    ///         assert_eq!(&200, &res.status);
    ///         assert_eq!(ToReturn {}, res.body.unwrap());
    ///     }
    /// ```
    pub async fn patch<T, U>(
        &self,
        endpoint: &str,
        data: Option<T>,
        options: Option<FetchOptions>,
    ) -> FetchResult<FetchResponse<U>>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let options = options.unwrap_or_default();
        let response = self
            .build_request(
                data,
                Some(&options),
                self.client.patch(self.build_url(endpoint, Some(&options))?),
            )?
            .send()
            .await
            .map_err(|e| FetchError::UnableToSendRequest { err: e })?;

        return Ok(self
            .response_to_fetch_response(response, options.deserialize_body)
            .await?);
    }
}
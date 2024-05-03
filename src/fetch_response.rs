use bytes::Bytes;
use std::net::SocketAddr;

use reqwest::StatusCode;

use crate::FetchHeaders;

#[derive(Debug)]
pub struct FetchResponse<T> {
    pub body: Option<T>,
    pub raw_body: Option<Bytes>,
    pub status: StatusCode,
    pub response_headers: FetchHeaders,
    pub remote_address: Option<SocketAddr>,
}

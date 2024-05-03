
use std::collections::HashMap;

use rust_fetch::{
    {ContentType, FetchOptions},
    map_string, Fetch, FetchConfig, USER_AGENT,
};
use httpmock::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct ToReturn {
    item1: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct NetworkTestResponse {
    item1: String,
    item2: String,
}

#[test]
fn test_set_default_headers() -> anyhow::Result<()> {
    let fetch = Fetch::default();

    assert_eq!(
        fetch
            .config
            .unwrap()
            .headers
            .unwrap()
            .get("user-agent")
            .unwrap(),
        USER_AGENT
    );
    Ok(())
}

#[test]
fn test_build_url_leading_slash() -> anyhow::Result<()> {
    let fetch = Fetch::new("http://localhost", None)?;

    let built_url = fetch.build_url("/v1/signup", None)?;
    assert_eq!("http://localhost/v1/signup", built_url.as_str());
    Ok(())
}

#[test]
fn test_build_url_no_leading_slash() -> anyhow::Result<()> {
    let fetch = Fetch::new("http://localhost", None)?;
    let built_url = fetch.build_url("v1/signup", None)?;

    assert_eq!("http://localhost/v1/signup", built_url.as_str());
    Ok(())
}

#[test]
fn test_build_url_with_params() -> anyhow::Result<()> {
    let fetch = Fetch::new("http://localhost", None).unwrap();
    let built_url = fetch.build_url(
        "/v1/signup",
        Some(&FetchOptions {
            params: Some(map_string! {param1 : "testParam1"}),
            ..Default::default()
        }),
    )?;

    assert_eq!(
        "http://localhost/v1/signup?param1=testParam1",
        built_url.as_str()
    );
    Ok(())
}

#[tokio::test]
async fn test_new_fetch_has_default_headers() -> anyhow::Result<()> {
    let server = MockServer::start();
    let fetch = Fetch::new(&server.base_url(), None)?;

    let mock = server.mock(|when, then| {
        when.path("/test")
            .header(
                reqwest::header::ACCEPT.to_string(),
                ContentType::Json.to_string(),
            )
            .header(
                reqwest::header::CONTENT_TYPE.to_string(),
                ContentType::Json.to_string(),
            );
        then.status(200);
    });

    fetch
        .post::<(), ()>(
            "/test",
            None,
            Some(FetchOptions {
                deserialize_body: false,
                ..Default::default()
            }),
        )
        .await?;

    mock.assert_async().await;
    Ok(())
}

#[tokio::test]
async fn test_fetch_json() -> anyhow::Result<()> {
    let server = MockServer::start();
    let fetch = Fetch::new(
        &server.base_url(),
        Some(FetchConfig {
            content_type: ContentType::Json,
            ..Default::default()
        }),
    )?;

    let mock = server.mock(|when, then| {
        when.path("/test").method(POST).header(
            reqwest::header::CONTENT_TYPE.to_string(),
            ContentType::Json.to_string(),
        );

        then.status(200);
    });

    fetch
        .post::<(), HashMap<String, String>>(
            "/test",
            Some(HashMap::new()),
            Some(FetchOptions {
                deserialize_body: false,
                accept: Some(ContentType::Json),
                ..Default::default()
            }),
        )
        .await?;

    mock.assert_async().await;
    Ok(())
}

#[tokio::test]
async fn test_fetch_xml() -> anyhow::Result<()> {
    let server = MockServer::start();
    let fetch = Fetch::new(
        &server.base_url(),
        Some(FetchConfig {
            accept: ContentType::Json,
            content_type: ContentType::ApplicationXml,
            ..Default::default()
        }),
    )?;
    let expected_response_body = NetworkTestResponse {
        item1: String::from("test"),
        item2: String::from("test2"),
    };
    let expected_request_body = ToReturn {
        item1: String::from("testing123"),
    };

    let mock = server.mock(|when, then| {
        when.path("/test")
            .method(POST)
            .header(
                reqwest::header::CONTENT_TYPE.to_string(),
                ContentType::ApplicationXml.to_string(),
            )
            .body(serde_xml_rs::to_string(&expected_request_body).unwrap());
        then.status(200).json_body_obj(&expected_response_body);
    });

    let res = fetch
        .post::<NetworkTestResponse, ToReturn>(
            "/test",
            Some(ToReturn {
                item1: String::from("testing123"),
            }),
            None,
        )
        .await?;
    mock.assert_async().await;
    assert_eq!(expected_response_body, res.body.unwrap());

    Ok(())
}

#[tokio::test]
async fn test_fetch_post() -> anyhow::Result<()> {
    let server = MockServer::start();
    let fetch = Fetch::new(&server.base_url(), None)?;

    let mock = server.mock(|when, then| {
        when.method(POST).path("/test");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                serde_json::to_string(&NetworkTestResponse {
                    item1: "Test".to_owned(),
                    item2: "Test2".to_owned(),
                })
                .unwrap(),
            );
    });

    let response = fetch
        .post::<NetworkTestResponse, ()>("/test", None, None)
        .await;

    assert_eq!(false, response.is_err());

    let response = response.unwrap();

    mock.assert_async().await;
    assert_eq!(&200, &response.status);
    assert_eq!(
        &NetworkTestResponse {
            item1: "Test".to_owned(),
            item2: "Test2".to_owned()
        },
        &response.body.unwrap()
    );
    assert_eq!(
        "application/json",
        response.response_headers.get("content-type").unwrap()
    );

    Ok(())
}

#[tokio::test]
async fn test_fetch_get() -> anyhow::Result<()> {
    let server = MockServer::start();
    let fetch = Fetch::new(&server.base_url(), None)?;

    let test_response = NetworkTestResponse {
        item1: "test1".to_string(),
        item2: "test2".to_string(),
    };

    let mock = server.mock(|when, then| {
        when.path("/test").method(GET).query_param("key", "value");
        then.body(serde_json::to_string(&test_response).unwrap())
            .status(200);
    });

    let response = fetch
        .get::<NetworkTestResponse>(
            "/test",
            Some(FetchOptions {
                headers: None,
                params: Some(map_string! {key : "value"}),
                ..Default::default()
            }),
        )
        .await?;

    mock.assert_async().await;
    assert_eq!(&200, &response.status);
    assert_eq!(&test_response, &response.body.unwrap());

    Ok(())
}

#[tokio::test]
async fn test_fetch_delete() -> anyhow::Result<()> {
    let server = MockServer::start();
    let fetch = Fetch::new(&server.base_url(), None)?;

    let to_return_obj = ToReturn {
        item1: "Test".to_string(),
    };

    server.mock(|when, then| {
        when.path("/test").method(DELETE);
        then.status(200).json_body_obj(&to_return_obj);
    });

    let res = fetch.delete::<(), ToReturn>("/test", None, None).await?;

    assert_eq!(&200, &res.status);
    assert_eq!(to_return_obj, res.body.unwrap());

    Ok(())
}

#[tokio::test]
async fn test_auto_deserialization_of_xml() -> anyhow::Result<()> {
    let server = MockServer::start();
    let fetch = Fetch::new(&server.base_url(), None)?;

    let expected_response = NetworkTestResponse {
        item1: String::from("testing"),
        item2: String::from("testing2"),
    };

    let mock = server.mock(|when, then| {
        when.path("/test").method(POST);
        let body = serde_xml_rs::to_string(&expected_response).unwrap();

        then.status(200)
            .header(
                reqwest::header::CONTENT_TYPE.to_string(),
                ContentType::ApplicationXml,
            )
            .body(body.as_bytes());
    });

    let res = fetch
        .post::<NetworkTestResponse, ()>("/test", None, None)
        .await?;

    mock.assert_async().await;
    assert_eq!(expected_response, res.body.unwrap());

    Ok(())
}

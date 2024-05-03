
# Rust Fetch

Rust Fetch is an easy-to-use HTTP client for making web requests in Rust applications. It provides a simple and minimalistic interface for sending HTTP requests, handling responses, and managing request options. 

## Key Features

- Easy-to-use API for sending POST, GET, DELETE, PUT, and PATCH requests
- Serialization and deserialization support for JSON, XML, and URL-encoded data
- Customizable request headers and query parameters
- Error handling for network errors, deserialization errors, and fetch errors
- Configurable request timeout and default headers



## Usage/Examples

Add the following to your `Cargo.toml` file:

```
[dependencies]
rust_fetch = "0.1.0"
```
basic example:

```rust
use rust_fetch::Fetch;

#[tokio::main]
async fn main() {
    let client = Fetch::new("http://localhost", None).unwrap();

    let response = client.post::<ToReturn, ToSend>(
        "/test",
        Some(ToSend {
            test_key: "Testing".to_string(),
        }),
        Some(rust_fetch::FetchOptions {
            params: Some(rust_fetch::map_string! { param1 : "value1" }),
            ..Default::default(),
        }),
    ).await.unwrap();

    assert_eq!(&200, &response.status);
    assert_eq!(ToReturn {}, response.body.unwrap());
}
```
## Contributions

Contributions are welcome! If you find any issues or have suggestions for improvements, feel free to submit a pull request or open an issue on the GitHub repository.

## License

This library is licensed under the MIT License. See the LICENSE file for more details.

Feel free to customize and enhance this README template to better suit the features and usage of your Rust Fetch library.

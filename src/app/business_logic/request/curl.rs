use anyhow::anyhow;
use regex::Regex;
use reqwest::header::CONTENT_TYPE;
use reqwest::Url;
use std::str::FromStr;
use thiserror::Error;

use crate::models::auth::auth::Auth;
use crate::models::auth::basic::BasicAuth;
use crate::models::auth::bearer_token::BearerToken;
use crate::models::auth::digest::{
    extract_www_authenticate_digest_data, Digest, DigestAlgorithm, DigestCharset, DigestQop,
};
use crate::models::protocol::http::body::ContentType;
use crate::models::protocol::http::body::ContentType::NoBody;
use crate::models::protocol::http::http::HttpRequest;
use crate::models::protocol::http::method::Method;
use crate::models::protocol::protocol::Protocol;
use crate::models::request::{KeyValue, Request};

#[derive(Error, Debug)]
enum ParseCurlError {
    #[error("Could not parse cURL\n\t{0}")]
    CouldNotParseCurl(String),
    #[error("Could not parse URL\n\t{0}")]
    CouldNotParseUrl(String),
    #[error("Unknown method\n\t{0}")]
    UnknownMethod(String),
}

/// Parse a cURL command into the request model used by both the CLI and TUI.
pub fn parse_curl_request(curl_string: &str, request_name: String) -> anyhow::Result<Request> {
    let normalized_curl = normalize_curl_options(curl_string);
    let parsed_curl = match curl_parser::ParsedRequest::load(&normalized_curl, None::<String>) {
        Ok(parsed_curl) => parsed_curl,
        Err(error) => return Err(anyhow!(ParseCurlError::CouldNotParseCurl(error.to_string()))),
    };

    /* URL */

    let mut curl_url = match Url::parse(&parsed_curl.url.to_string()) {
        Ok(url) => url,
        Err(error) => return Err(anyhow!(ParseCurlError::CouldNotParseUrl(error.to_string()))),
    };

    /* QUERY PARAMS */

    // Request stores the base URL and query parameters separately. Collect the
    // pairs before removing the query from the URL.
    let params = curl_url
        .query_pairs()
        .map(|(key, value)| KeyValue {
            enabled: true,
            data: (key.to_string(), value.to_string()),
        })
        .collect();

    curl_url.set_query(None);
    let url = curl_url.to_string();

    /* METHOD */

    let method = match Method::from_str(parsed_curl.method.as_str()) {
        Ok(method) => method,
        Err(error) => return Err(anyhow!(ParseCurlError::UnknownMethod(error.to_string()))),
    };

    /* HEADERS */

    let headers: Vec<KeyValue> = parsed_curl
        .headers
        .iter()
        .filter(|(header_name, _)| !header_name.as_str().eq_ignore_ascii_case("authorization"))
        .map(|(key, value)| KeyValue {
            enabled: true,
            data: (key.to_string(), value.to_str().unwrap_or_default().to_string()),
        })
        .collect();

    /* AUTH */

    let basic_auth_regex = Regex::new(r#"(-u|--user) ["'](?<username>.*):(?<password>.*)["']"#)?;
    let digest_auth_regex = Regex::new(r#"--digest (-u|--user) ["'](?<username>.*):(?<password>.*)["']"#)?;

    let auth = match basic_auth_regex.captures(curl_string) {
        None => {
            let authorization_header_value = parsed_curl
                .headers
                .iter()
                .find_map(|(header_name, value)| {
                    header_name
                        .as_str()
                        .eq_ignore_ascii_case("authorization")
                        .then(|| value.to_str().unwrap_or_default())
                });

            let digest_credentials = digest_auth_regex.captures(curl_string).map(|capture| {
                (capture["username"].to_string(), capture["password"].to_string())
            });

            match authorization_header_value {
                None => match digest_credentials {
                    None => Auth::NoAuth,
                    Some((username, password)) => Auth::Digest(Digest {
                        username,
                        password,
                        domains: String::new(),
                        realm: String::new(),
                        nonce: String::new(),
                        opaque: String::new(),
                        stale: false,
                        algorithm: DigestAlgorithm::default(),
                        qop: DigestQop::default(),
                        user_hash: false,
                        charset: DigestCharset::default(),
                        nc: 0,
                    }),
                },
                Some(authorization_header_value) => {
                    if let Some(token) = authorization_header_value.strip_prefix("Bearer ") {
                        Auth::BearerToken(BearerToken { token: token.to_string() })
                    } else if authorization_header_value.starts_with("Digest ") {
                        let (username, password) = digest_credentials.unwrap_or_default();
                        let (domains, realm, nonce, opaque, stale, algorithm, qop, user_hash, charset) =
                            extract_www_authenticate_digest_data(authorization_header_value)?;

                        Auth::Digest(Digest {
                            username,
                            password,
                            domains,
                            realm,
                            nonce,
                            opaque,
                            stale,
                            algorithm,
                            qop,
                            user_hash,
                            charset,
                            nc: 0,
                        })
                    } else {
                        Auth::NoAuth
                    }
                }
            }
        }
        Some(capture) => Auth::BasicAuth(BasicAuth {
            username: capture["username"].to_string(),
            password: capture["password"].to_string(),
        }),
    };

    /* BODY */

    let body = if !parsed_curl.body.is_empty() {
        let body_string = parsed_curl.body.join("\n");
        let content_type = headers
            .iter()
            .find(|header| header.data.0.eq_ignore_ascii_case(CONTENT_TYPE.as_str()));

        match content_type {
            Some(content_type) => ContentType::from_content_type(&content_type.data.1, body_string),
            None => NoBody,
        }
    } else {
        NoBody
    };

    Ok(Request {
        name: request_name,
        url,
        params,
        headers,
        auth,
        protocol: Protocol::HttpRequest(HttpRequest { method, body }),
        ..Default::default()
    })
}

/// Normalize common cURL aliases that are not understood by `curl-parser`.
///
/// The parser supports `--data`, `--header`, and similar core options, but
/// copied commands commonly use `--data-raw` and `--cookie`. Keep the shell
/// quoting intact while translating those options to equivalent forms.
fn normalize_curl_options(curl_string: &str) -> String {
    const DATA_ALIASES: [&str; 4] = ["--data-raw", "--data-binary", "--data-ascii", "--data-urlencode"];
    const COOKIE_ALIASES: [&str; 2] = ["--cookie", "-b"];

    let chars: Vec<char> = curl_string.chars().collect();
    let mut normalized = String::with_capacity(curl_string.len());
    let mut quote: Option<char> = None;
    let mut escaped_in_double_quote = false;
    let mut token_start = true;
    let mut cookie_value_pending = false;
    let mut index = 0;

    while index < chars.len() {
        let character = chars[index];

        if let Some(quote_character) = quote {
            normalized.push(character);

            if quote_character == '"' {
                if escaped_in_double_quote {
                    escaped_in_double_quote = false;
                } else if character == '\\' {
                    escaped_in_double_quote = true;
                } else if character == quote_character {
                    quote = None;
                }
            } else if character == quote_character {
                quote = None;
            }

            index += 1;
            continue;
        }

        if cookie_value_pending {
            if character.is_whitespace() {
                normalized.push(character);
                index += 1;
                continue;
            }

            if character == '\\' && chars.get(index + 1).is_some_and(|next| *next == '\n' || *next == '\r') {
                normalized.push(character);
                index += 1;
                continue;
            }

            if character == '\'' || character == '"' {
                normalized.push(character);
                normalized.push_str("Cookie: ");
                quote = Some(character);
                token_start = false;
                cookie_value_pending = false;
                index += 1;
                continue;
            }

            normalized.push_str("Cookie: ");
            cookie_value_pending = false;
        }

        if character == '\'' || character == '"' {
            normalized.push(character);
            quote = Some(character);
            token_start = false;
            index += 1;
            continue;
        }

        if character.is_whitespace() {
            normalized.push(character);
            token_start = true;
            index += 1;
            continue;
        }

        if token_start {
            let data_alias = DATA_ALIASES.iter().find(|alias| {
                starts_with_token(&chars, index, alias)
            });
            if let Some(alias) = data_alias {
                normalized.push_str("--data");
                index += alias.chars().count();
                token_start = false;
                continue;
            }

            let cookie_alias = COOKIE_ALIASES.iter().find(|alias| {
                starts_with_token(&chars, index, alias)
            });
            if let Some(alias) = cookie_alias {
                normalized.push_str("--header");
                index += alias.chars().count();
                token_start = false;
                cookie_value_pending = true;
                continue;
            }
        }

        normalized.push(character);
        token_start = false;
        index += 1;
    }

    normalized
}

fn starts_with_token(chars: &[char], start: usize, token: &str) -> bool {
    let token_chars: Vec<char> = token.chars().collect();
    let end = start + token_chars.len();

    chars.get(start..end) == Some(token_chars.as_slice())
        && chars.get(end).is_none_or(|character| character.is_whitespace())
}

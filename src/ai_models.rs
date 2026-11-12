//! Read model catalogs from the same connection used to generate drafts.

use crate::{ai, ai_cli, hosting::secret_get};
use serde_json::Value;
use std::{collections::HashSet, io::Read, time::Duration};

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Model {
    pub id: String,
    pub name: String,
}

impl Model {
    pub fn from_fields(row: &Value, id: &str, name: &str) -> Option<Self> {
        let id = row[id].as_str()?.trim();
        if id.is_empty() {
            return None;
        }
        let name = row[name]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(id);
        Some(Self {
            id: id.into(),
            name: name.into(),
        })
    }
}

pub fn list(config: &ai::AiConfig) -> Result<Vec<Model>, String> {
    let mut models = if let Some(agent) = ai_cli::Agent::from_provider(&config.provider) {
        ai_cli::list_models(agent, &config.cli_path, &config.cli_config_dir)?
    } else {
        let key = secret_get(ai::key_name(&config.provider)).filter(|k| !k.trim().is_empty());
        list_api(config, key.as_deref())?
    };
    let mut seen = HashSet::new();
    models.retain(|model| seen.insert(model.id.clone()));
    Ok(models)
}

fn catalog_url(config: &ai::AiConfig) -> Result<tauri::Url, String> {
    let mut url =
        tauri::Url::parse(&ai::api_url(config)?).map_err(|_| "Invalid AI endpoint URL.")?;
    let suffix = if config.provider == "anthropic" {
        "/messages"
    } else {
        "/chat/completions"
    };
    let base = url
        .path()
        .strip_suffix(suffix)
        .ok_or("Could not locate the model list for this endpoint.")?;
    let path = format!("{base}/models");
    url.set_path(&path);
    Ok(url)
}

fn list_api(config: &ai::AiConfig, key: Option<&str>) -> Result<Vec<Model>, String> {
    let mut url = catalog_url(config)?;
    let provider = config.provider.as_str();
    let name = ai::provider_label(provider);
    if key.is_none() && !matches!(provider, "ollama" | "custom") {
        return Err(format!(
            "Save your {name} API key first, then fetch models."
        ));
    }
    // Keep the credential on the selected endpoint, even if it redirects.
    let client = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(12))
        .build();
    let anthropic = provider == "anthropic";
    if anthropic && !url.query_pairs().any(|(k, _)| k == "limit") {
        url.query_pairs_mut().append_pair("limit", "100");
    }
    let mut models = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..10 {
        let mut page = url.clone();
        if let Some(cursor) = &cursor {
            page.query_pairs_mut().append_pair("after_id", cursor);
        }
        let mut request = client.get(page.as_str()).set("Accept", "application/json");
        if let Some(key) = key {
            request = if anthropic {
                request
                    .set("x-api-key", key)
                    .set("anthropic-version", ai::API_VERSION)
            } else {
                request.set("Authorization", &format!("Bearer {key}"))
            };
        }
        let response = request.call().map_err(|error| match error {
            ureq::Error::Status(401 | 403, _) => format!("{name} rejected access to the model list. Check the saved API key."),
            ureq::Error::Status(404 | 405, _) => format!("{name} does not expose a model list at this endpoint. You can still enter a model ID manually."),
            ureq::Error::Status(code, _) => format!("{name} returned HTTP {code} while listing models."),
            _ => format!("Could not reach {name} to list models. Check the endpoint and try again."),
        })?;
        if !(200..300).contains(&response.status()) {
            return Err(format!(
                "{name} redirected the model request. Set the endpoint to the final address."
            ));
        }
        let body: Value = serde_json::from_reader(response.into_reader().take(4 * 1024 * 1024))
            .map_err(|_| format!("{name} did not return a readable model list."))?;
        let rows = body["data"].as_array().ok_or_else(|| {
            format!("{name} did not return a model list. You can enter a model ID manually.")
        })?;
        models.extend(rows.iter().filter_map(|row| {
            let mut model = Model::from_fields(row, "id", "display_name")?;
            if provider == "google" {
                if let Some(id) = model.id.strip_prefix("models/") {
                    if model.name == model.id {
                        model.name = id.to_owned();
                    }
                    model.id = id.to_owned();
                }
            }
            Some(model)
        }));
        if !anthropic || body["has_more"] != true {
            return Ok(models);
        }
        let next = body["last_id"]
            .as_str()
            .ok_or("The model list did not include its next page.")?;
        if cursor.as_deref() == Some(next) {
            return Err("The model server returned the same page twice.".into());
        }
        cursor = Some(next.into());
    }
    Err("The model list exceeded the page limit. Enter a model ID manually.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
    };

    #[test]
    fn catalogs_use_the_configured_api_base_and_keep_proxy_queries() {
        for (provider, endpoint, expected) in [
            ("openai", "", "https://api.openai.com/v1/models"),
            (
                "google",
                "",
                "https://generativelanguage.googleapis.com/v1beta/openai/models",
            ),
            (
                "ollama",
                "http://localhost:11434/api",
                "http://localhost:11434/v1/models",
            ),
            (
                "custom",
                "https://example.com/proxy/v1/chat/completions?api-version=1",
                "https://example.com/proxy/v1/models?api-version=1",
            ),
            (
                "anthropic",
                "https://example.com/proxy/v1/messages",
                "https://example.com/proxy/v1/models",
            ),
        ] {
            let config = ai::AiConfig {
                provider: provider.into(),
                endpoint: endpoint.into(),
                ..Default::default()
            };
            assert_eq!(catalog_url(&config).unwrap().as_str(), expected);
        }
    }

    #[test]
    fn requests_anthropic_pages_with_the_saved_key_and_reads_display_names() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/proxy/v1", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for (query, body) in [
                (
                    "limit=100",
                    r#"{"data":[{"id":"one","display_name":"Model One"}],"has_more":true,"last_id":"one"}"#,
                ),
                (
                    "limit=100&after_id=one",
                    r#"{"data":[{"id":"two"}],"has_more":false}"#,
                ),
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                let request: Vec<_> = BufReader::new(&mut socket)
                    .lines()
                    .map(Result::unwrap)
                    .take_while(|l| !l.is_empty())
                    .collect();
                assert_eq!(request[0], format!("GET /proxy/v1/models?{query} HTTP/1.1"));
                assert!(request
                    .iter()
                    .any(|line| line.eq_ignore_ascii_case("x-api-key: test-key")));
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let config = ai::AiConfig {
            endpoint,
            ..Default::default()
        };
        let models = list_api(&config, Some("test-key")).unwrap();
        assert_eq!(
            models,
            [
                Model {
                    id: "one".into(),
                    name: "Model One".into()
                },
                Model {
                    id: "two".into(),
                    name: "two".into()
                }
            ]
        );
        server.join().unwrap();
    }
}

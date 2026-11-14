//! Follow collection continuations without sending credentials to another origin.
use crate::hosting::HttpPage;
use serde_json::Value;

fn origin(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return String::new() };
    format!("{}://{}", scheme.to_ascii_lowercase(), rest.split('/').next().unwrap_or_default().to_ascii_lowercase())
}

fn parameter(url: &str, key: &str) -> Option<String> {
    url.split_once('?')?.1.split('&').find_map(|part| part.split_once('=').filter(|(name, _)| *name == key).map(|(_, value)| value.to_owned()))
}

fn with_parameter(url: &str, key: &str, value: &str) -> String {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let mut parts: Vec<_> = query.split('&').filter(|part| !part.is_empty() && part.split('=').next() != Some(key)).map(str::to_owned).collect();
    parts.push(format!("{key}={value}"));
    format!("{base}?{}", parts.join("&"))
}

fn array_key(value: &Value) -> Option<&str> {
    if value.is_array() { return Some(""); }
    ["values", "value", "items", "issues"].into_iter().find(|key| value[*key].is_array())
}

fn next(url: &str, page: &HttpPage, count: usize) -> Option<String> {
    if let Some(link) = &page.link {
        if let Some(part) = link.split(',').find(|part| part.contains("rel=\"next\"")) {
            if let Some((_, rest)) = part.split_once('<') { return rest.split_once('>').map(|(url, _)| url.to_owned()); }
        }
    }
    if let Some(value) = page.value["next"].as_str().filter(|s| !s.is_empty()) { return Some(value.into()); }
    if let Some(value) = page.next_page.as_deref().filter(|s| !s.is_empty()) { return Some(with_parameter(url, "page", value)); }
    if let Some(value) = page.continuation.as_deref().filter(|s| !s.is_empty()) { return Some(with_parameter(url, "continuationToken", &crate::hosting_api::q(value))); }
    if page.value["isLastPage"] == false {
        if let Some(start) = page.value["nextPageStart"].as_u64() { return Some(with_parameter(url, "start", &start.to_string())); }
    }
    if let (Some(start), Some(total), Some(limit)) = (page.value["startAt"].as_u64(), page.value["total"].as_u64(), page.value["maxResults"].as_u64()) {
        if count > 0 && start + limit < total { return Some(with_parameter(url, "startAt", &(start + limit).to_string())); }
    }
    if let Some(size) = parameter(url, "per_page").and_then(|s| s.parse::<usize>().ok()) {
        if size > 0 && count >= size {
            let page = parameter(url, "page").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
            return Some(with_parameter(url, "page", &(page + 1).to_string()));
        }
    }
    None
}

pub fn collect(start: &str, mut fetch: impl FnMut(&str) -> Result<HttpPage, String>) -> Result<Value, String> {
    let mut current = start.to_owned();
    let mut visited = std::collections::HashSet::new();
    let mut combined: Option<Value> = None;
    loop {
        if visited.len() >= 1000 { return Err("The service exceeded 1000 result pages; narrow the query.".into()); }
        if !visited.insert(current.clone()) { return Err("The service repeated a pagination URL; results may be incomplete.".into()); }
        let page = fetch(&current)?;
        let Some(key) = array_key(&page.value) else { return Ok(page.value) };
        let rows = if key.is_empty() { page.value.as_array().unwrap() } else { page.value[key].as_array().unwrap() };
        let following = next(&current, &page, rows.len());
        if let Some(total) = &mut combined {
            let destination = if key.is_empty() { total.as_array_mut() } else { total[key].as_array_mut() }.ok_or("The service changed its pagination format.")?;
            destination.extend(rows.iter().cloned());
        } else { combined = Some(page.value); }
        let Some(following) = following else { return Ok(combined.unwrap()); };
        let following = if following.starts_with('/') { format!("{}{following}", origin(start)) } else { following };
        if origin(&following) != origin(start) { return Err("The service returned a pagination URL on a different host; credentials were not forwarded.".into()); }
        current = following;
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn page(value:Value)->HttpPage { HttpPage {value,link:None,next_page:None,continuation:None} }
    #[test]
    fn collects_all_pages_and_rejects_foreign_hosts_and_loops() {
        let mut calls=0;
        let result=collect("https://api.example/repos?per_page=2", |url| {
            calls+=1;
            Ok(if parameter(url, "page").as_deref() == Some("2") {page(json!([3]))} else {page(json!([1,2]))})
        }).unwrap();
        assert_eq!(result,json!([1,2,3]));assert_eq!(calls,2);
        let mut calls=0;
        assert!(collect("https://api.example/repos", |_| { calls+=1; Ok(page(json!({"values":[],"next":"https://evil.example/repos"}))) }).is_err());
        assert_eq!(calls,1);
        assert!(collect("https://api.example/repos", |_| Ok(page(json!({"values":[],"next":"https://api.example/repos"})))).is_err());
    }
    #[test]
    fn follows_server_offset_and_continuation_headers() {
        let mut calls=0;
        let value=collect("https://api.example/list?limit=1", |url| {
            calls+=1;
            Ok(if url.contains("start=1") {page(json!({"values":[2],"isLastPage":true}))} else {page(json!({"values":[1],"isLastPage":false,"nextPageStart":1}))})
        }).unwrap();assert_eq!(value["values"],json!([1,2]));assert_eq!(calls,2);
        let mut calls=0;
        let value=collect("https://api.example/list", |url| {
            calls+=1; let mut p=page(json!({"value":[calls]}));
            if calls==1 {p.continuation=Some("a+b".into());} else {assert!(url.contains("continuationToken=a%2Bb"));}
            Ok(p)
        }).unwrap();assert_eq!(value["value"],json!([1,2]));
    }
}

use std::{env, collections::HashMap};

use reqwest::{Response, header::{HeaderMap, HeaderValue}};
use serde::Serialize;
use serde_json::Value;

use crate::db::auth::auth_info;

use super::{config::bitbucket_base_url, auth::update_access_token};

#[derive(Serialize)]
struct Comment {
    content: Content,
}

#[derive(Serialize)]
struct Content {
    raw: String,
}
pub async fn add_comment(workspace_name: &str, repo_name: &str, review_id: &str, comment_text: &str) {
    let url = format!("{}/repositories/{workspace_name}/{repo_name}/pullrequests/{review_id}/comments", bitbucket_base_url());
    println!("comment url = {}", &url);
    let auth_info = auth_info();
    let mut access_token = auth_info.access_token().clone();
    let new_auth_opt = update_access_token(&auth_info).await;
    if new_auth_opt.is_some() {
        let new_auth = new_auth_opt.expect("new_auth_opt is empty");
        access_token = new_auth.access_token().clone();
    }
    let comment_payload = Comment {
        content: Content {
            raw: comment_text.to_string(),
        },
    };
    let client = reqwest::Client::new();
    let mut headers = reqwest::header::HeaderMap::new(); 
    headers.insert( reqwest::header::AUTHORIZATION, 
    format!("Bearer {}", access_token).parse().expect("Invalid auth header"), );
    headers.insert("Accept",
     "application/json".parse().expect("Invalid Accept header"));
    let response_res = client.post(&url).
        headers(headers).json(&comment_payload).send().await;
    if response_res.is_err() {
        eprintln!("Error in post request for adding commen - {:?}", response_res.as_ref().expect_err("response has no error"));
    }
    let response = response_res.expect("Error in getting response");
    if response.status().is_success() {
        eprintln!("Failed to call API {}, status: {}", url, response.status());
    }
    let response_json = response.json::<Value>().await;
    println!("response from comment api = {:?}", &response_json);
}
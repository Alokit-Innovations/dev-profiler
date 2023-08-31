use std::env;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::{task, fs};

use crate::bitbucket::auth::get_access_token_from_bitbucket;
use crate::bitbucket::repo::get_workspace_repos;
use crate::bitbucket::workspace::get_bitbucket_workspaces;
use crate::bitbucket::webhook::{get_webhooks_in_repo, add_webhook};
use crate::db::repo::save_repo_to_db;
use crate::db::webhook::save_webhook_to_db;
use crate::utils::repo::Repository;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SetupInfo {
    provider: String,
    owner: String,
    repos: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PublishRequest {
    installation_id: String,
    info: Vec<SetupInfo>,
}

pub async fn handle_install_bitbucket(installation_code: &str) {
    // get access token from installation code by calling relevant repo provider's api
    // out of github, bitbucket, gitlab

    let authinfo = get_access_token_from_bitbucket(installation_code).await.expect("Unable to get access token");
    println!("AuthInfo: {:?}", authinfo);
    // let auth_info = { "access_token": access_token, "expires_in": expires_in_formatted, "refresh_token": auth_info["refresh_token"] }; db.insert("auth_info", serde_json::to_string(&auth_info).unwrap());
    let access_token = authinfo.access_token().clone();
    let user_workspaces = get_bitbucket_workspaces(&access_token).await;
    let mut pubreqs: Vec<SetupInfo> = Vec::new();
    for workspace in user_workspaces {
        let workspace_slug = workspace.slug();
        println!("=========<{:?}>=======", workspace_slug);
    
        let repos = get_workspace_repos(workspace.uuid(), 
            &access_token).await;
        let mut reponames: Vec<String> = Vec::new();
        for repo in repos.expect("repos is None") {
            let token_copy = access_token.clone();
            let mut repo_copy = repo.clone();
            clone_git_repo(&mut repo_copy, &token_copy).await;
            let repo_name = repo.name();
            reponames.push(repo_name.clone());
            println!("Repo url git = {:?}", &repo.clone_ssh_url());
            println!("Repo name = {:?}", repo_name);
            process_webhooks(workspace_slug.to_string(),
            repo_name.to_string(),
            access_token.to_string()).await;
            let repo_name_async = repo_name.clone();
            let workspace_slug_async = workspace_slug.clone();
            let access_token_async = access_token.clone();
            // task::spawn(async move {
            //     get_prs(&workspace_slug_async,
            //         &repo_name_async,
            //         &access_token_async,
            //         "OPEN").await;
            // });
        }
        pubreqs.push(SetupInfo {
            provider: "bitbucket".to_owned(),
            owner: workspace_slug.clone(),
            repos: reponames });
    } 
    send_setup_info(&pubreqs).await;
}

async fn send_setup_info(setup_info: &Vec<SetupInfo>) {
    let installation_id = env::var("INSTALL_ID")
        .expect("INSTALL_ID must be set");
    let base_url = env::var("SERVER_URL")
        .expect("SERVER_URL must be set");
    let body = PublishRequest {
        installation_id: installation_id,
        info: setup_info.to_vec(),
    };
    let client = Client::new();
    let resp = client
      .post(format!("{base_url}/api/rustapp/setup"))
      .json(&body)
      .send()
      .await
      .unwrap();

    println!("Response: {}", resp.text().await.unwrap());
}

async fn clone_git_repo(repo: &mut Repository, access_token: &str) {
    let git_url = repo.clone_ssh_url();
    let clone_url = git_url.to_string()
        .replace("git@", format!("https://x-token-auth:{{{access_token}}}@").as_str())
        .replace("bitbucket.org:", "bitbucket.org/");
    let random_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    let mut directory = format!("/tmp/{}/{}/{}", repo.provider(), repo.workspace(), random_string);
    // Check if directory exists
    if fs::metadata(&directory).await.is_ok() {
        fs::remove_dir_all(&directory).await.expect("Unable to remove pre-existing directory components");
    }
    fs::create_dir_all(&directory).await.expect("Unable to create directory");
    println!("directory exists? {}", fs::metadata(&directory).await.is_ok());
    let mut cmd = std::process::Command::new("git");
    cmd.arg("clone").arg(clone_url).current_dir(&directory);
    let output = cmd.output().expect("Failed to clone git repo");
    println!("Git clone output: {:?}", output);
    directory = format!("{}/{}", &directory, repo.name());
    repo.set_local_dir(directory);
    save_repo_to_db(repo);
}

async fn process_webhooks(workspace_slug: String, repo_name: String, access_token: String) {
    let webhooks_data = get_webhooks_in_repo(
        &workspace_slug, &repo_name, &access_token).await;
    let webhook_callback_url = format!("{}/api/bitbucket/callbacks/webhook", 
        env::var("SERVER_URL").expect("SERVER_URL must be set"));
    match webhooks_data.is_empty() {
        true => { 
            let repo_name_async = repo_name.clone();
            let workspace_slug_async = workspace_slug.clone();
            let access_token_async = access_token.clone();
            task::spawn(async move {
                add_webhook(
                    &workspace_slug_async, 
                    &repo_name_async, 
                    &access_token_async).await;
            });
        },
        false => {
            let matching_webhook = webhooks_data.into_iter()
                .find(|w| w.url().to_string() == webhook_callback_url);
            if matching_webhook.is_some() {
                let webhook = matching_webhook.expect("no matching webhook");
                println!("Webhook already exists: {:?}", &webhook);
                save_webhook_to_db(&webhook);
            } else {
                println!("Adding new webhook...");
                let repo_name_async = repo_name.clone();
                let workspace_slug_async = workspace_slug.clone();
                let access_token_async = access_token.clone();
                task::spawn(async move {
                    add_webhook(
                        &workspace_slug_async, 
                        &repo_name_async, 
                        &access_token_async).await;
                });
            }
        },
    };
}
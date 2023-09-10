use crate::db::auth::auth_info;
use crate::db::user::save_user_to_db;
use crate::utils::auth::AuthInfo;
use crate::utils::user::{User, Provider, ProviderEnum};
use super::config::{bitbucket_base_url, get_api};

pub async fn get_and_save_workspace_users(workspace_id: &str, access_token: &str) {
    let base_url = bitbucket_base_url();
    let members_url = format!("{}/workspaces/{}/members", &base_url, workspace_id);
    let response_json = get_api(&members_url, access_token, None).await;
    for user_json in response_json {
        let provider_id = user_json["user"]["uuid"].to_string().replace('"', "");
        let user = User::new(
            Provider::new(
                provider_id,
                ProviderEnum::Bitbucket),
        user_json["user"]["display_name"].to_string().replace('"', ""),
        user_json["workspace"]["slug"].to_string().replace('"', ""),
        None);
        save_user_to_db(&user);
    }
}

pub async fn get_commit_bb(commit: &str, repo_name: &str, repo_owner: &str) {
    let base_url = bitbucket_base_url();
    let commits_url = format!("{}/repositories/{repo_owner}/{repo_name}/commit/{commit}", &base_url);
    println!("commits url = {}", &commits_url);
    let authinfo: AuthInfo =  auth_info();
    let access_token = authinfo.access_token();
    let response_json = get_api(&commits_url, access_token, None).await;
    println!("response json for commits url = {:?}", &response_json);
}
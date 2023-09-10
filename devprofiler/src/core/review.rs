use std::env;

use serde_json::Value;

use crate::{utils::{hunk::{HunkMap, PrHunkItem}, review::Review, gitops::{commit_exists, git_pull, get_excluded_files, generate_diff, process_diff, generate_blame}}, db::{hunk::{get_hunk_from_db, store_hunkmap_to_db}, repo::get_clone_url_clone_dir, review::{save_review_to_db, self}}, core::coverage::process_coverage};

pub async fn process_review(message_data: &Vec<u8>) {
	let review_opt = get_tasks(message_data);
	if review_opt.is_none() {
		eprintln!("No review tasks found!");
		return;
	}
	let review = review_opt.expect("review_opt is empty");
	let hunk = get_hunk_from_db(&review);
	if hunk.is_some() {
		let hunkval = hunk.expect("hunk is empty");
		publish_hunkmap(&hunkval);
		eprintln!("Hunk already in db!");
		return;
	}
	let mut prvec = Vec::<PrHunkItem>::new();
	println!("Processing PR : {}", review.id());
	if !commit_exists(&review.base_head_commit()) || !commit_exists(&review.pr_head_commit()) {
		println!("Pulling repository {} for commit history", review.repo_name());
		git_pull(&review).await;
	}
	let fileopt = get_excluded_files(&review);
	println!("fileopt = {:?}", &fileopt);
	if fileopt.is_none() {
		eprintln!("No files to review for PR {}", review.id());
		return;
	}
	let (_, smallfiles) = fileopt.expect("fileopt is empty");
	let diffmap = generate_diff(&review, &smallfiles);
	println!("diffmap = {:?}", &diffmap);
	let linemap = process_diff(&diffmap);
	let blamevec = generate_blame(&review, &linemap).await;
	let hmapitem = PrHunkItem::new(
		review.id().to_string(),
		review.author().to_string(),
		blamevec,
	);
	prvec.push(hmapitem);
	let hunkmap = HunkMap::new(review.provider().to_string(),
		review.repo_owner().to_string(), 
		review.repo_name().to_string(), 
		prvec,
		format!("{}/hunkmap", review.db_key()),
	);
	store_hunkmap_to_db(&hunkmap, &review);
	publish_hunkmap(&hunkmap);
	let hunkmap_async = hunkmap.clone();
	process_coverage(&hunkmap_async).await;
}

fn get_tasks(message_data: &Vec<u8>) -> Option<Review>{
	match serde_json::from_slice::<Value>(&message_data) {
		Ok(data) => {
			println!("data == {:?}", &data["eventPayload"]["repository"]);
			let repo_provider = data["repositoryProvider"].to_string().trim_matches('"').to_string();
			let repo_name = data["eventPayload"]["repository"]["name"].to_string().trim_matches('"').to_string();
			println!("repo NAME == {}", &repo_name);
			let workspace_name = data["eventPayload"]["repository"]["workspace"]["slug"].to_string().trim_matches('"').to_string();
			let (clone_url, clone_dir) = get_clone_url_clone_dir(&repo_provider, &workspace_name, &repo_name);
			let pr_id = data["eventPayload"]["pullrequest"]["id"].to_string().trim_matches('"').to_string();
			let review = Review::new(
                data["eventPayload"]["pullrequest"]["destination"]["commit"]["hash"].to_string().replace("\"", ""),
				data["eventPayload"]["pullrequest"]["source"]["commit"]["hash"].to_string().replace("\"", ""),
				pr_id.clone(),
                repo_name.clone(),
                workspace_name.clone(),
				repo_provider.clone(),
				format!("bitbucket/{}/{}/{}", &workspace_name, &repo_name, &pr_id),
				clone_dir,
				clone_url,
				data["eventPayload"]["pullrequest"]["author"]["account_id"].to_string().replace("\"", ""),
            );
			println!("review = {:?}", &review);
			save_review_to_db(&review);
			return Some(review);
		},
		Err(e) => {eprintln!("Incoming message does not contain valid reviews: {e}");},
	};
	return None;
}

fn publish_hunkmap(hunkmap: &HunkMap) {
	let client = reqwest::Client::new();
	let hunkmap_json = serde_json::to_string(&hunkmap).expect("Unable to serialize hunkmap");
	tokio::spawn(async move {
		let url = format!("{}/api/hunks",
			env::var("SERVER_URL").expect("SERVER_URL must be set"));
		println!("url for hunkmap publishing  {}", &url);
		match client
		.post(url)
		.json(&hunkmap_json)
		.send()
		.await {
			Ok(_) => {
				println!("Hunkmap published successfully!");
			},
			Err(e) => {
				eprintln!("Failed to publish hunkmap: {}", e);
			}
		};
	});
}

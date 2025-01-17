use std::{cell::LazyCell, collections::HashMap, hash::Hash, io::BufReader, path::{Path, PathBuf}, str::FromStr};

use tokio::fs;
use clap::{Parser, Subcommand};
use log::{info, warn};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use serde::Deserialize;

const MAX_PKG_SIZE: u64 = 100 * 1024 * 1024;

const API_TPLS: LazyCell<HashMap<&str, HashMap<&str, &str>>> = LazyCell::new(||{
    vec![
        ("repo.prefix.dev", vec![
            ("upload", "https://prefix.dev/api/v1/upload/$channel"),
            ("delete", "https://prefix.dev/api/v1/delete/$channel/$subdir/$pkg"),
        ].into_iter().collect())
    ].into_iter().collect()
});

#[derive(Debug, Parser)] // requires `derive` feature
#[command(name = "prefix_api")]
#[command(about = "package manager api for prefix.dev", long_about = None)]
struct Cli {
    #[arg(short = 'k', long, default_value="~/.mamba/auth/authentication.json")]
    token: String,
    #[arg(short, long, default_value="repo.prefix.dev")]
    repo: String,
    #[arg(short, long, default_value="vidlg")]
    channel: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Upload {
        #[arg()]
        pkgs: Vec<PathBuf>,
    },
    Delete {
        #[arg()]
        pkgs: Vec<PathBuf>,
    }
}

#[derive(Debug, Deserialize)]
struct AuthValue {
    token: String,
    #[serde(rename = "type")]
    type_: String,
}

async fn upload_pkg(pkg_path: &Path, token: &str, upload_url: &str) {
    let metadata = fs::metadata(pkg_path).await.unwrap();
    if metadata.len() > MAX_PKG_SIZE {
        warn!("Skipping {pkg} because it is too large!", pkg=pkg_path.display());
    }

    let pkg_name = pkg_path.file_name().unwrap().to_str().unwrap();
    let sha256 = &sha256::try_digest(pkg_path).unwrap();

    let body = fs::read(pkg_path).await.unwrap();

    info!("uploading pkg {pkg_name} to {upload_url}");
    let client = reqwest::Client::new();
    let res = client.post(upload_url)
        .bearer_auth(token)
        .header(CONTENT_LENGTH,body.len())
        .header(CONTENT_TYPE, "application/octet-stream")
        .header("X-File-Name", pkg_name)
        .header("X-File-SHA256", sha256)
        .body(body)
        .send()
        .await
        .unwrap()
    ;
    let status_code = res.status();
    info!("uploaded pkg {pkg_name} to {upload_url} with status {status_code}");
}

async fn delete_pkg(pkg_path: &Path, token: &str, delete_url: &str, tpl_vars: &HashMap<&str, &str>) {
    let subdir = pkg_path.parent().unwrap().file_name().unwrap().to_str().unwrap();
    let pkg_name = pkg_path.file_name().unwrap().to_str().unwrap();

    let mut vars = tpl_vars.clone();
    vars.insert("subdir", subdir);
    vars.insert("pkg", pkg_name);

    println!("{subdir} {pkg_name}");

    let delete_url = &subst::substitute(delete_url, &vars).unwrap();

    info!("deleting pkg {pkg_name} to {delete_url}");
    let client = reqwest::Client::new();
    let res = client.delete(delete_url)
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
    ;
    let status_code = res.status();
    info!("deleted pkg {pkg_name} to {delete_url} with status code {status_code}");
}

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let args = Cli::parse();
    let Cli{token, repo, channel, command, ..} = &args;
    let repo = repo.as_str();
    let tpl_vars: HashMap<&str, &str> = vec![
        ("repo", repo),
        ("token", token),
        ("channel", channel),
    ].into_iter().collect();

    let token = if token.ends_with(".json") {
        let token = shellexpand::tilde(token);
        let token: &Path = token.as_ref().as_ref();
        let json_str = String::from_utf8(fs::read(token).await.unwrap()).unwrap();
        let auth_data: HashMap<String, AuthValue> = serde_json::from_str(&json_str).unwrap();
        auth_data.get(repo).unwrap().token.clone()
    } else {
        token.into()
    };

    let api_tpls = API_TPLS.clone();
    match command {
        Commands::Upload{
            pkgs, ..
        } => {
            let upload_url = api_tpls.get(repo).unwrap().get("upload").unwrap();
            let upload_url = subst::substitute(upload_url, &tpl_vars).unwrap();
            for i in pkgs {
                upload_pkg(i, &token, &upload_url).await;
            }
        },
        Commands::Delete { pkgs, .. } => {
            let delete_url = api_tpls.get(repo).unwrap().get("delete").unwrap();

            for i in pkgs {
                delete_pkg(i, &token, &delete_url, &tpl_vars).await;
            }
        }
    }
}

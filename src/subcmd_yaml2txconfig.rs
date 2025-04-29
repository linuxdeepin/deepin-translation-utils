use core::panic;
use std::fs;
use std::path::PathBuf;
use std::io::stdin;
use directories::{BaseDirs, ProjectDirs};
use thiserror::Error as TeError;

use crate::transifex_rest_api::{TransifexData, TransifexRestApi};
use crate::transifex_yaml_file::*;
use crate::tx_config_file::load_transifexrc_file;

#[derive(TeError, Debug)]
pub enum CmdY2TCError {
    #[error("Fail to load transifex.yaml file because: {0}")]
    TxYamlLoadError(#[from] TxYamlLoadError),
}

fn get_github_repository_from_user_input(project_root: &PathBuf, github_repository_hint: Option<String>) -> String {
    let project_root = fs::canonicalize(project_root).unwrap_or(project_root.to_path_buf());
    let mut repo_name = match github_repository_hint {
        Some(github_repository_hint_name) => github_repository_hint_name,
        None => project_root.file_name().and_then(|name| name.to_str().map(ToOwned::to_owned)).unwrap_or(String::new()),
    };

    loop {
        if repo_name.contains('/') && repo_name.split('/').count() == 2 {
            return repo_name.to_string();
        }

        let github_repository = format!("{}/{}", "linuxdeepin", repo_name);
        println!("Is {github_repository:?} your GitHub repo name?\n- If yes, simply press Enter.\n- If not, please enter the repo name in owner/repo format: ");
        let mut user_input = String::new();
        repo_name = match stdin().read_line(&mut user_input) {
            Ok(_) => {
                let user_input = user_input.trim();
                if user_input.is_empty() {
                    github_repository
                } else {
                    user_input.to_string().trim().to_owned()
                }
            },
            Err(_) => {
                println!("Failed to read user input.");
                panic!();
            }
        }
    }
}

fn create_rest_client_from_transifexrc_file(force_online: bool) -> TransifexRestApi {
    let xdg_dirs = BaseDirs::new().expect("Not able to get xdg base directories");
    let transifexrc_file = xdg_dirs.home_dir().join(".transifexrc");
    if !transifexrc_file.exists() {
        eprintln!("Warning: .transifexrc file not found, not able to fetch project resource list");
        if force_online {
            panic!("Force online mode, but .transifexrc file not found");
        }
    }
    let transifexrc = load_transifexrc_file(&transifexrc_file).expect("Failed to load .transifexrc file");
    TransifexRestApi::new(&transifexrc.rest_hostname, &transifexrc.token)
}

fn fetch_project_list(organization_slug: &str, force_online: bool) -> Vec<String> {
    let xdg_proj_dirs = ProjectDirs::from("", "deepin", "deepin-translation-utils").expect("Not able to get project directories");
    let cache_file = xdg_proj_dirs.cache_dir().join(format!("{organization_slug}.yaml"));
    
    if cache_file.exists() && !force_online {
        let source_content = fs::read_to_string(&cache_file).expect("Failed to read cached project list");
        let list = serde_yml::from_str::<Vec<String>>(source_content.as_str()).expect("Failed to parse cached project list");
        return list;
    } else {
        let client = create_rest_client_from_transifexrc_file(force_online);

        println!("Fetching o:{organization_slug} project list from Transifex...");
        let entries = client.get_all_projects(organization_slug).expect("Failed to fetch project resource list");
        let entries = entries.into_iter().map(|entry| entry.id.to_string());
        let entries: Vec<String> = entries.collect();
        let cache_content = serde_yml::to_string::<Vec<String>>(&entries).expect("Failed to serialize project list as cache");
        let parent_dir = cache_file.parent().expect("Failed to get cache file parent directory");
        fs::create_dir_all(&parent_dir).expect("Failed to create cache directory");
        fs::write(&cache_file, cache_content).expect("Failed to write cache file");
        return entries;
    }
}

fn parse_linked_resource_category(entry: TransifexData) -> Option<TxResourceLookupEntry> {
    let binding = entry.attributes.categories?;
    let category = binding.iter()
        .find(|&c| c.starts_with("github#repository:"))?;

    let re = regex::Regex::new(r"^github#repository:(?P<organization>[^/]+)/(?P<repository>[^#]+)#branch:(?P<branch>[^#]+)#path:(?P<path>.+)$").unwrap();
    let captures = re.captures(category)?;
    let organization = captures.name("organization")?.as_str();
    let repository = captures.name("repository")?.as_str();
    let branch = captures.name("branch")?.as_str();
    let path = captures.name("path")?.as_str();
    Some(TxResourceLookupEntry {
        repository: format!("{organization}/{repository}"),
        branch: branch.to_string(),
        resource: path.to_string(),
        transifex_resource_id: entry.id.to_string(),
    })
}

fn fetch_resource_list(organization_slug: &str, project_slug: &str, force_online: bool) -> Vec<TxResourceLookupEntry> {
    let xdg_proj_dirs = ProjectDirs::from("", "deepin", "deepin-translation-utils").expect("Not able to get project directories");
    let cache_file = xdg_proj_dirs.cache_dir().join(format!("{organization_slug}/{project_slug}.yaml"));
    
    if cache_file.exists() && !force_online {
        println!("Reusing o:{organization_slug}:p:{project_slug} project resource list from local cache...");
        let source_content = fs::read_to_string(&cache_file).expect("Failed to read cached project resource list");
        let list = serde_yml::from_str::<Vec<TxResourceLookupEntry>>(source_content.as_str()).expect("Failed to parse cached project resource list");
        return list;
    } else {
        let client = create_rest_client_from_transifexrc_file(force_online);

        println!("Fetching o:{organization_slug}:p:{project_slug} project resource list from Transifex...");
        let entries = client.get_all_linked_resources(organization_slug, project_slug).expect("Failed to fetch project resource list");
        let entries = entries.into_iter().filter_map(parse_linked_resource_category).collect();
        let cache_content = serde_yml::to_string::<Vec<TxResourceLookupEntry>>(&entries).expect("Failed to serialize project resource list as cache");
        let parent_dir = cache_file.parent().unwrap();
        fs::create_dir_all(&parent_dir).expect("Failed to create cache directory");
        fs::write(&cache_file, cache_content).expect(format!("Failed to write project cache file to {cache_file:?}").as_str());
        return entries;
    }
}

pub fn subcmd_yaml2txconfig(project_root: &PathBuf, force_online: bool, github_repository: Option<String>, organization_slug: String, project_slug: Option<String>) -> Result<(), CmdY2TCError> {
    let (transifex_yaml_file, tx_yaml) = try_laod_transifex_yaml_file(project_root)?;
    println!("Found Transifex project config file at: {transifex_yaml_file:?}");

    let github_repository = get_github_repository_from_user_input(project_root, github_repository);
    println!("GitHub repository name: {github_repository}");
    
    let mut lookup_table = Vec::<TxResourceLookupEntry>::new();

    if let Some(project_slug) = project_slug {
        let resource_list = fetch_resource_list(&organization_slug, &project_slug, force_online);
        lookup_table.extend(resource_list);
    } else {
        let project_list = fetch_project_list(&organization_slug, force_online);
        for project_full_slug in project_list {
            // project_full_slug is in the format of o:linuxdeepin:p:deepin-home
            // use regex to extract project_slug
            let re = regex::Regex::new(r"^o:(?P<organization>[^:]+):p:(?P<project>[^:]+)$").unwrap();
            let captures = re.captures(&project_full_slug).unwrap();
            let project_slug = captures.name("project").unwrap().as_str();
            let resource_list = fetch_resource_list(&organization_slug, &project_slug, force_online);
            lookup_table.extend(resource_list);
        }
    }

    let tx_config = tx_yaml.to_tx_config(github_repository, lookup_table);

    let tx_config_file = project_root.join(".tx/config");
    if tx_config_file.exists() {
        println!("Note: {tx_config_file:?} file already exists, not overwriting it.");
        println!("You can use the following context to update the file manually:\n");
        println!("{}", tx_config.to_str());
    } else {
        let parent_dir = tx_config_file.parent().unwrap();
        fs::create_dir_all(&parent_dir).expect("Failed to create .tx directory");
        fs::write(&tx_config_file, tx_config.to_str()).expect("Failed to write .tx/config file");
        println!("Generated .tx/config file at: {tx_config_file:?}");
    }

    Ok(())
}
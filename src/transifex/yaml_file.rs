// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

// transifex.yaml file spec: https://help.transifex.com/en/articles/6265125-github-installation-and-configuration#h_94380d9cd8

use std::{fs, path::PathBuf};

use regex::Regex;
use serde::{Serialize, Deserialize};
use thiserror::Error as TeError;

use super::tx_config_file::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct TransifexYaml {
    pub filters: Vec<Filter>,
    pub settings: Settings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TxResourceLookupEntry {
    pub repository: String,
    /// Git branch name, not transifex branch name
    pub branch: String,
    pub resource: String,
    /// Full slug, i.e. `o:org:p:proj:r:res`
    pub transifex_resource_id: String,
}

impl TransifexYaml {
    pub fn to_tx_config(&self, github_repository: String, lookup_table: Vec<TxResourceLookupEntry>) -> TxConfig {
        let mut resource_sections = Vec::<TxConfigSectionResource>::new();
        let mut unknown_count = 0; // avoid duplicate resource name when attempting to convert to .tx/config file
        for filter in &self.filters {
            let mut resource_section = TxConfigSectionResource::default();
            resource_section.source_file = filter.source.clone();
            resource_section.source_lang = filter.source_lang.clone();
            resource_section.type_attr = filter.format.clone();
            resource_section.file_filter = filter.target_pattern.clone();

            // from lookup table, find if we have resource have the same repository and resource name
            if let Some(lookup_entry) = lookup_table.iter().find(|entry| {
                entry.repository == github_repository && entry.resource == filter.source
            }) {
                resource_section.resource_full_slug = lookup_entry.transifex_resource_id.clone();
            } else {
                unknown_count += 1;
                resource_section.resource_full_slug = format!("o:{}:p:{}:r:{}-{}", "unknown-org", "unknown-proj", "unknown-res", unknown_count);
            }
            
            resource_sections.push(resource_section);
        };
        TxConfig {
            main_section: TxConfigSectionMain {
                host: "https://www.transifex.com".to_string(),
                ..TxConfigSectionMain::default()
            },
            resource_sections,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Filter {
    #[serde(rename = "filter_type")]
    pub type_attr: String,
    #[serde(rename = "source_file")]
    pub source: String,
    #[serde(rename = "file_format")]
    pub format: String,
    #[serde(rename = "source_language")]
    pub source_lang: String,
    #[serde(rename = "translation_files_expression")]
    pub target_pattern: String,
}

impl Filter {
    pub fn match_target_files(&self, project_root: &PathBuf) -> Result<Vec<(String, PathBuf)>, std::io::Error> {
        let target_pattern_path = project_root.join(&self.target_pattern);
        let Some(target_filename_pattern) = target_pattern_path.file_name() else {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "File name not found"));
        };
        let Some(target_filename_pattern) = target_filename_pattern.to_str() else {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "File name not valid"));
        };
        if target_filename_pattern.contains("<lang>") {
            let Some(target_filter_pattern) = create_filter_pattern(target_filename_pattern) else {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Filter pattern not valid"));
            };
            let Some(target_parent) = target_pattern_path.parent() else {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Parent dir not found"));
            };
            let target_files = target_parent.read_dir()?;
            let mut matched_files = Vec::<(String, PathBuf)>::new();
            for file in target_files {
                let file = file?;
                let file_name = file.file_name();
                let Some(file_name) = file_name.to_str() else {
                    continue;
                };
                target_filter_pattern.captures(file_name).and_then(|captures| {
                    captures.get(1).map(|lang_code| {
                        let lang_code = lang_code.as_str();
                        matched_files.push((lang_code.to_string(), file.path()));
                    })
                });
            };
            Ok(matched_files)
        } else {
            // target_pattern_path is something like `./path/to/<lang>/the/file.ext`
            // let's get the basedir before <lang> (i.e. `./path/to/`), then match folders under that path
            // `<lang>` is a language code.
            // then get file based on the matched folders, e.g. `./path/to/es/the/file.ext` and `./path/to/zh_CN/the/file.ext`
            // if `<lang>` is not a part of the path, return error.
            let mut parent_dir = PathBuf::new();
            let mut remain_path : Option<PathBuf> = None;
            let mut components = target_pattern_path.components();
            // while components.next() is not <lang>, push to parent_dir
            while let Some(component) = components.next() {
                if let std::path::Component::Normal(normal_path) = component {
                    if normal_path != "<lang>" {
                        parent_dir.push(normal_path);
                    } else {
                        remain_path = Some(components.as_path().to_path_buf());
                        break;
                    }
                } else {
                    parent_dir.push(component);
                }
            };
            if remain_path.is_none() {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Missing <lang> inside the pattern."));
            }
            let remain_path = remain_path.unwrap();
            let language_folders = parent_dir.read_dir()?;
            let mut matched_files = Vec::<(String, PathBuf)>::new();
            let language_code_regex = regex::Regex::new(r"[a-z_A-Z]{2,6}").unwrap();
            for language_folder in language_folders {
                // check if language_folder is a valid language code ([a-z_A-Z]{{2,6}}) in regex
                if let Ok(language_folder) = language_folder {
                    let language_folder_dir = language_folder.path();
                    let language_folder = language_folder.file_name();
                    let Some(language_folder) = language_folder.to_str() else {
                        continue;
                    };
                    if !language_code_regex.is_match(language_folder) {
                        continue;
                    }
                    let matched_file = language_folder_dir.join(&remain_path);
                    if !matched_file.is_file() {
                        continue;
                    }
                    matched_files.push((language_folder.to_string(), matched_file));
                }
            }
            Ok(matched_files)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(rename = "pr_branch_name")]
    pub branch_template: String,
}

#[derive(TeError, Debug)]
pub enum LoadTxYamlError {
    #[error("File not found")]
    FileNotFound,
    #[error("Can not read file")]
    ReadFile(#[from] std::io::Error),
    #[error("Fail to deserialize file: {0}")]
    Serde(#[from] serde_yml::Error),
    #[error("Fail to convert from .tx/config file: {0:?}")]
    ConvertFile(#[from] LoadTxConfigError),
}

pub fn try_load_transifex_yaml_file(project_root: &PathBuf) -> Result<(PathBuf, TransifexYaml), LoadTxYamlError> {
    // try find transifex.yaml in project_root/transifex.yaml and if not found, try project_root/.tx/transifex.yaml. If still not found, return error.
    let transifex_yaml_file = project_root.join("transifex.yaml");
    if transifex_yaml_file.is_file() {
        let tx_yaml = load_tx_yaml_file(&transifex_yaml_file)?;
        return Ok((transifex_yaml_file, tx_yaml));
    }
    let transifex_yaml_file = project_root.join(".tx").join("transifex.yaml");
    if transifex_yaml_file.is_file() {
        let tx_yaml = load_tx_yaml_file(&transifex_yaml_file)?;
        return Ok((transifex_yaml_file, tx_yaml));
    }

    Err(LoadTxYamlError::FileNotFound)
}

pub fn load_tx_yaml_file(transifex_yaml_file: &PathBuf) -> Result<TransifexYaml, LoadTxYamlError> {
    if !transifex_yaml_file.is_file() {
        return Err(LoadTxYamlError::FileNotFound);
    }
    let source_content = fs::read_to_string(&transifex_yaml_file)?;
    Ok(serde_yml::from_str::<TransifexYaml>(source_content.as_str())?)
}

fn create_filter_pattern(pattern: &str) -> Option<Regex> {
    let parts: Vec<&str> = pattern.split("<lang>").collect();
    if parts.len() != 2 {
        return None;
    }

    let regex_pattern = format!(
        r#"^{}([a-z_A-Z]{{2,6}}){}$"#,
        regex::escape(parts[0]),
        regex::escape(parts[1])
    );

    Regex::new(&regex_pattern).ok()
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const TEST_TX_YAML_CONTENT: &str = r#"# Some comments or spdx license headers
filters:
  - filter_type: file
    source_file: shell-launcher-applet/translations/org.deepin.ds.dock.launcherapplet.ts
    file_format: QT
    source_language: en_US
    translation_files_expression: shell-launcher-applet/translations/org.deepin.ds.dock.launcherapplet_<lang>.ts
  - filter_type: file
    source_file: dcc-network/translations/network_en_US.ts
    file_format: QT
    source_language: en_US
    translation_files_expression: dcc-network/translations/network_<lang>.ts
settings:
  pr_branch_name: transifex_update_<br_unique_id>
"#;

    #[test]
    fn tst_parse_tx_yaml_content() {
        let tx_yaml: TransifexYaml = serde_yml::from_str::<TransifexYaml>(TEST_TX_YAML_CONTENT).unwrap();
        assert_eq!(tx_yaml.filters.len(), 2);
        assert_eq!(tx_yaml.filters[0].type_attr, "file");
        assert_eq!(tx_yaml.filters[0].source, "shell-launcher-applet/translations/org.deepin.ds.dock.launcherapplet.ts");
        assert_eq!(tx_yaml.filters[0].format, "QT");
        assert_eq!(tx_yaml.filters[0].source_lang, "en_US");
        assert_eq!(tx_yaml.filters[0].target_pattern, "shell-launcher-applet/translations/org.deepin.ds.dock.launcherapplet_<lang>.ts");
    }

    #[test]
    fn tst_convert_to_tx_config() {
        let tx_yaml: TransifexYaml = serde_yml::from_str::<TransifexYaml>(TEST_TX_YAML_CONTENT).unwrap();
        let tx_config = tx_yaml.to_tx_config("user/repo".to_string(), vec![]);
        assert_eq!(tx_config.resource_sections[0].resource_full_slug, "o:unknown-org:p:unknown-proj:r:unknown-res-1");
        assert_eq!(tx_config.resource_sections[0].file_filter, "shell-launcher-applet/translations/org.deepin.ds.dock.launcherapplet_<lang>.ts");
        assert_eq!(tx_config.resource_sections[1].resource_full_slug, "o:unknown-org:p:unknown-proj:r:unknown-res-2");
    }

    #[test]
    fn test_pathbuf() {
        let path = PathBuf::from("/example/sample_<lang>.ts");
        assert_eq!(path.file_name(), Some(std::ffi::OsStr::new("sample_<lang>.ts")));
        let pattern = create_filter_pattern(path.to_str().unwrap()).unwrap();
        let matched = pattern.captures("/example/sample_zh_CN.ts").and_then(|caps| caps.get(1)).map(|m| {
            m.as_str().to_string()
        });
        assert_eq!(matched, Some("zh_CN".to_string()));
    }
}

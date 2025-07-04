// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use serde::Serialize;
use thiserror::Error as TeError;
use std::path::{Path, PathBuf};
use crate::transifex::{yaml_file::*, tx_config_file::*};
use crate::i18n_file::{self, common::{MessageStats, I18nFileKind}};

#[derive(TeError, Debug)]
pub enum CmdError {
    #[error("Can not guess translation file kind from path {0:?} because: {1}")]
    GuessI18nFileType(PathBuf, #[source] i18n_file::common::UnknownI18nFileExtError),
    #[error("Fail to load Qt Linguist TS file {0:?} because: {1}")]
    LoadTsFile(PathBuf, #[source] i18n_file::linguist::TsLoadError),
    #[error("Fail to load Gettext PO/POT file {0:?} because: {1}")]
    LoadPoFile(PathBuf, #[source] i18n_file::gettext::PoLoadError),
    #[error("Fail to load Transifex project file because: {0}")]
    LoadTxProjectFile(#[from] TxProjectFileLoadError),
    #[error("Fail to match resources because: {0}")]
    MatchResources(#[source] std::io::Error),
    #[error("Fail to serialize stats to YAML: {0}")]
    SerdeYaml(#[from] serde_yml::Error),
    #[error("Fail to serialize stats to JSON: {0}")]
    SerdeJson(#[from] serde_json::Error),
}

#[derive(clap::ValueEnum, Clone, Default, Copy, Debug)]
pub enum StatsFormat {
    #[default]
    PlainTable,
    Yaml,
    Json,
}

#[derive(clap::ValueEnum, Clone, Default, Copy, Debug)]
pub enum StatsSortBy {
    LanguageCode,
    #[default]
    Completeness,
}

#[derive(Default, Serialize)]
struct ProjectResourceStats {
    project_path: PathBuf,
    target_lang_codes: Vec<String>,
    resource_groups: Vec<TsResourceGroupStats>,
}

fn load_file_stats(file_path: &Path) -> Result<MessageStats, CmdError> {
    let kind = i18n_file::common::I18nFileKind::from_ext_hint(&file_path)
        .map_err(|e| CmdError::GuessI18nFileType(file_path.to_path_buf(), e))?;

    Ok(match kind {
        I18nFileKind::Linguist => i18n_file::linguist::Ts::load_from_file(&file_path)
            .map_err(|e| CmdError::LoadTsFile(file_path.to_path_buf(), e))?
            .get_message_stats(),
        I18nFileKind::Gettext => i18n_file::gettext::Po::load_from_file(&file_path)
            .map_err(|e| CmdError::LoadPoFile(file_path.to_path_buf(), e))?
            .get_message_stats(),
    })
}

impl ProjectResourceStats {
    pub fn get_source_stats(&self) -> (i32, MessageStats) {
        let mut total_resources = 0;
        let mut total_stats = MessageStats::default();
        for resource_group in &self.resource_groups {
            total_stats += &resource_group.source_stats;
            total_resources += 1;
        };
        (total_resources, total_stats)
    }

    pub fn get_target_stats_by_language_code(&self, language_code: &String) -> (i32, MessageStats) {
        let mut total_resources = 0;
        let mut total_stats = MessageStats::default();
        for resource_group in &self.resource_groups {
            if let Some(target_stats) = resource_group.target_stats.get(language_code) {
                total_stats += &target_stats.stats;
                total_resources += 1;
            }
        };
        (total_resources, total_stats)
    }

    pub fn print_state_plain_table(&self, standalone_percentage: bool, sort_by: StatsSortBy) {
        println!("| No. | Lang   | Completeness | Resources | Translated | Unfinished | Vanished |");
        println!("| --- | ------ | ------------ | --------- | ---------- | ---------- | -------- |");
        let (source_resources, source_stats) = self.get_source_stats();
        let total_strings = source_stats.shown_translated() + source_stats.shown_unfinished();
        let reference_total = (!standalone_percentage).then_some(total_strings);
        println!("|   0 | Source | {0:>11.2}% | {1:9} | {2:10} | {3:10} | {4:8} |", 
            100.0, source_resources, total_strings, 0, source_stats.shown_obsolete());
        let language_codes = match sort_by {
            StatsSortBy::LanguageCode => {
                self.target_lang_codes.clone()
            }
            StatsSortBy::Completeness => {
                let mut sorted_langs = self.target_lang_codes.clone();
                sorted_langs.sort_by(|a, b| {
                    let (_, a_stats) = self.get_target_stats_by_language_code(&a);
                    let (_, b_stats) = self.get_target_stats_by_language_code(&b);
                    let a_completeness = a_stats.completeness_percentage(reference_total);
                    let b_completeness = b_stats.completeness_percentage(reference_total);
                    if a_completeness > b_completeness {
                        std::cmp::Ordering::Less
                    } else if a_completeness < b_completeness {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Equal
                    }
                });
                sorted_langs
            }
        };
        
        for (idx, lang) in language_codes.iter().enumerate() {
            let (target_resources, target_stats) = self.get_target_stats_by_language_code(&lang);
            println!("| {0:3} | {1:>6} | {2:>11.2}% | {3:9} | {4:10} | {5:10} | {6:8} |", 
                idx + 1, lang, target_stats.completeness_percentage(reference_total), target_resources, target_stats.shown_translated(), target_stats.shown_unfinished(), target_stats.shown_obsolete());
        }
    }

    pub fn print_stats_yaml(&self) -> Result<(), serde_yml::Error> {
        let yaml_str = serde_yml::to_string::<Self>(self)?;
        println!("{}", yaml_str);
        Ok(())
    }

    pub fn print_stats_json(&self) -> Result<(), serde_json::Error> {
        let json_str = serde_json::to_string_pretty(self)?;
        println!("{}", json_str);
        Ok(())
    }
}

#[derive(Default, Serialize)]
struct TsResourceGroupStats {
    source_path: PathBuf,
    source_lang_code: String,
    source_stats: MessageStats,
    target_lang_codes: Vec<String>,
    target_stats: std::collections::HashMap<String, TsResourceStats>,
}

#[derive(Default, Serialize)]
struct TsResourceStats {
    resource_path: PathBuf,
    stats: MessageStats,
}

#[derive(TeError, Debug)]
pub enum TxProjectFileLoadError {
    #[error("Fail to load transifex.yaml file because: {0}")]
    TxYamlLoadError(#[from] LoadTxYamlError),
    #[error("Fail to load .tx/config project file because: {0}")]
    ConvertError(#[from] LoadTxConfigError),
}

/// Try find transifex.yaml in `project_root/transifex.yaml`.
/// And if not found, try `project_root/.tx/transifex.yaml`.
/// If still not found, return error.
fn try_load_transifex_project_file(project_root: &PathBuf) -> Result<(PathBuf, TransifexYaml), TxProjectFileLoadError> {
    try_load_transifex_yaml_file(project_root).or_else(|e| {
        try_load_tx_config_file(project_root).map(|(tx_config_file, tx_config)| {
            let tx_yaml = tx_config.to_transifex_yaml();
            (tx_config_file, tx_yaml)
        }).map_err(|_| TxProjectFileLoadError::TxYamlLoadError(e))
    })
}

pub fn subcmd_statistics(project_root: &PathBuf, format: StatsFormat, sort_by: StatsSortBy, standalone_percentage: bool, accept_languages: Vec<String>, ignore_languages: Vec<String>) -> Result<(), CmdError> {
    let (transifex_yaml_file, tx_yaml) = try_load_transifex_project_file(project_root)?;
    if matches!(format, StatsFormat::PlainTable) {
        println!("Found Transifex project config file at: {transifex_yaml_file:?}");
    }
    let mut project_stats = ProjectResourceStats::default();
    project_stats.project_path = project_root.clone();

    for filter in &tx_yaml.filters {
        if (filter.format != "QT" && filter.format != "PO") || filter.type_attr != "file" {
            if matches!(format, StatsFormat::PlainTable) {
                println!("Skipping resource {:?} with format {:?}...", filter.source, filter.format);
            }
            continue;
        }
        let mut source_group_stats = TsResourceGroupStats::default();
        let source_file = project_root.join(&filter.source);
        // check if project_root/filter.source_file exists, and print stats of the source file if exists.
        if source_file.is_file() {
            if matches!(format, StatsFormat::PlainTable) {
                println!("Hit source file at: {source_file:?}");
            }
            let content_stats = load_file_stats(&source_file)?;
            source_group_stats.source_path = source_file.clone();
            source_group_stats.source_lang_code = filter.source_lang.clone();
            source_group_stats.source_stats = content_stats;
        } else {
            if matches!(format, StatsFormat::PlainTable) {
                println!("Missing source resource: {source_file:?}");
            }
            continue;
        }

        let matched_resources = filter.match_target_files(project_root).or_else(|e| { Err(CmdError::MatchResources(e)) })?;
        for (lang, target_file) in matched_resources {
            if !accept_languages.is_empty() && !accept_languages.contains(&lang) {
                continue;
            }
            if ignore_languages.contains(&lang) {
                continue;
            }
            let content_stats = load_file_stats(&target_file)?;
            let target_resource_stats = TsResourceStats {
                resource_path: target_file.clone(),
                stats: content_stats,
            };
            source_group_stats.target_lang_codes.push(lang.clone());
            if !project_stats.target_lang_codes.contains(&lang) {
                project_stats.target_lang_codes.push(lang.clone());
            }
            source_group_stats.target_stats.insert(lang, target_resource_stats);
        }

        project_stats.resource_groups.push(source_group_stats);
    }
    project_stats.target_lang_codes.sort();

    // finally, print the stats of the project
    match format {
        StatsFormat::PlainTable => project_stats.print_state_plain_table(standalone_percentage, sort_by),
        StatsFormat::Yaml => project_stats.print_stats_yaml()?,
        StatsFormat::Json => project_stats.print_stats_json()?,
    }

    Ok(())
}
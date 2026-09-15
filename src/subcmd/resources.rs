// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

//! Shared translation resource discovery helpers for subcommands which scan
//! the project following `transifex.yaml` / `.tx/config`.

use std::path::{Path, PathBuf};
use thiserror::Error as TeError;

use crate::i18n_file::common::{I18nFileKind, UnknownI18nFileExtError};
use crate::transifex::project_file::{try_load_transifex_project_file, TxProjectFileLoadError};

#[derive(TeError, Debug)]
pub enum ResourceError {
    #[error("Fail to load Transifex project file because: {0}")]
    LoadTxProjectFile(#[from] TxProjectFileLoadError),
    #[error("Fail to match resources because: {0}")]
    MatchResources(#[source] std::io::Error),
    #[error("Can not guess translation file kind from path {0:?} because: {1}")]
    GuessI18nFileType(PathBuf, #[source] UnknownI18nFileExtError),
}

/// Guess the translation file kind of the given path, returning `ts` or `po`.
pub fn lang_kind_from_path(path: &Path) -> Result<String, ResourceError> {
    match I18nFileKind::from_ext_hint(path) {
        Ok(I18nFileKind::Linguist) => Ok("ts".to_string()),
        Ok(I18nFileKind::Gettext) => Ok("po".to_string()),
        Err(e) => Err(ResourceError::GuessI18nFileType(path.to_path_buf(), e)),
    }
}

/// Collect the target-language resource file paths for every supported filter.
///
/// When `accept_languages` is empty, resources of all languages are collected.
pub fn collect_resource_paths(
    project_root: &PathBuf,
    accept_languages: &[String],
) -> Result<Vec<PathBuf>, ResourceError> {
    let (_, tx_yaml) = try_load_transifex_project_file(project_root)?;
    let mut rv = Vec::new();
    for filter in &tx_yaml.filters {
        if (filter.format != "QT" && filter.format != "PO") || filter.type_attr != "file" {
            continue;
        }
        let matched = filter
            .match_target_files(project_root)
            .map_err(ResourceError::MatchResources)?;
        rv.extend(
            matched
                .into_iter()
                .filter_map(|(lang, path)| {
                    (accept_languages.is_empty() || accept_languages.iter().any(|l| l == &lang))
                        .then_some(path)
                }),
        );
    }
    Ok(rv)
}

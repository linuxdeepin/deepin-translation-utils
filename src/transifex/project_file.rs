// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use thiserror::Error as TeError;

use std::path::PathBuf;
use crate::transifex::{yaml_file::*, tx_config_file::*};

#[derive(TeError, Debug)]
pub enum TxProjectFileLoadError {
    #[error("Fail to load transifex.yaml file because: {0}")]
    TxYamlLoadError(#[from] LoadTxYamlError),
    #[error("Fail to load .tx/config project file because: {0}")]
    ConvertError(#[from] LoadTxConfigError),
}

/// Try find transifex.yaml in `project_root/transifex.yaml`.
/// And if not found, try `project_root/.tx/transifex.yaml`.
/// And if not found, try `project_root/.tx/config`.
/// If still not found, return error.
pub fn try_load_transifex_project_file(project_root: &PathBuf) -> Result<(PathBuf, TransifexYaml), TxProjectFileLoadError> {
    try_load_transifex_yaml_file(project_root).or_else(|e| {
        try_load_tx_config_file(project_root).map(|(tx_config_file, tx_config)| {
            let tx_yaml = tx_config.to_transifex_yaml();
            (tx_config_file, tx_yaml)
        }).map_err(|_| TxProjectFileLoadError::TxYamlLoadError(e))
    })
}
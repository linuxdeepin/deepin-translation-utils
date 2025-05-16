// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use std::fs;
use std::path::PathBuf;
use thiserror::Error as TeError;

use crate::transifex::{yaml_file::*, tx_config_file::*};

#[derive(TeError, Debug)]
pub enum CmdError {
    #[error("Fail to load .tx/config file because: {0}")]
    LoadTxConfig(#[from] LoadTxConfigError),
    #[error("Fail to save transifex.yaml file because: {0}")]
    SaveTransifexYaml(#[from] serde_yml::Error),
}

pub fn subcmd_txconfig2yaml(project_root: &PathBuf) -> Result<(), CmdError> {
    let (tx_config_path, tx_config) = try_load_tx_config_file(project_root)?;
    let tx_yaml = tx_config.to_transifex_yaml();
    let tx_yaml_path = tx_config_path.parent().unwrap().join("transifex.yaml");
    if tx_yaml_path.exists() {
        println!("Note: {tx_yaml_path:?} file already exists, not overwriting it.");
        println!("You can use the following context to update the file manually:\n");
        println!("{}", serde_yml::to_string::<TransifexYaml>(&tx_yaml)?);
    } else {
        fs::write(&tx_yaml_path, serde_yml::to_string::<TransifexYaml>(&tx_yaml)?).unwrap();
        println!("Wrote transifex.yaml file to: {}", tx_yaml_path.display());
    }

    Ok(())
}
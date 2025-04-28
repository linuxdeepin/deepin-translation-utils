use std::fs;
use std::path::PathBuf;
use thiserror::Error as TeError;

use crate::transifex_yaml_file::*;
use crate::tx_config_file::*;

#[derive(TeError, Debug)]
pub enum CmdTC2YError {
    #[error("Fail to load .tx/config file because: {0}")]
    TxConfigLoadError(#[from] TxConfigLoadError),
    #[error("Fail to save transifex.yaml file because: {0}")]
    TransifexYamlSaveError(#[from] serde_yml::Error),
}

pub fn subcmd_txconfig2yaml(project_root: &PathBuf) -> Result<(), CmdTC2YError> {
    let (tx_config_path, tx_config) = try_laod_tx_config_file(project_root)?;
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
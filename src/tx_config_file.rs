// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

// .transifexrc content: https://github.com/transifex/cli/blob/devel/examples/exampleconf/.transifexrc
// .tx/config file spec: https://developers.transifex.com/docs/using-the-client

use std::{fs, path::PathBuf};
use configparser::ini::{Ini, WriteOptions};
use thiserror::Error as TeError;
use crate::transifex_yaml_file::{self, TransifexYaml};

#[derive(TeError, Debug)]
pub enum TxConfigLoadError {
    #[error("File not found")]
    FileNotFound,
    #[error("Can not read file")]
    ReadFile(#[from] std::io::Error),
    #[error("Fail to deserialize file: {0}")]
    ParseError(String),
}

#[derive(Default)]
pub struct TransifexRcSection {
    pub host_section: String,
    pub rest_hostname: String,
    pub token: String,
}

pub fn load_transifexrc_file(transifexrc_file: &PathBuf) -> Result<TransifexRcSection, TxConfigLoadError> {
    if !transifexrc_file.is_file() {
        return Err(TxConfigLoadError::FileNotFound);
    }
    let source_content = fs::read_to_string(&transifexrc_file)?;
    TransifexRcSection::from_str(&source_content)
}

impl TransifexRcSection {
    pub fn from_str(content: &str) -> Result<Self, TxConfigLoadError> {
        let mut config = Ini::new();
        config.read(content.to_string())
          .map_err(|err| TxConfigLoadError::ParseError(err.to_string()))?;

        let mut tx_section = TransifexRcSection::default();

        let sections = config.sections();
        for section in sections {
            tx_section.host_section = section.to_string();
            tx_section.rest_hostname = config.get(&section, "rest_hostname").ok_or(TxConfigLoadError::ParseError("missing rest_hostname key".to_string()))?;
            tx_section.token = config.get(&section, "token").ok_or(TxConfigLoadError::ParseError("missing token key".to_string()))?;

            break;
        };
        Ok(tx_section)
    }
}

#[derive(Default)]
pub struct TxConfig {
    pub main_section: TxConfigSectionMain,
    pub resource_sections: Vec<TxConfigSectionResource>,
}

pub fn load_tx_config_file(tx_config_file: &PathBuf) -> Result<TxConfig, TxConfigLoadError> {
    if !tx_config_file.is_file() {
        return Err(TxConfigLoadError::FileNotFound);
    }
    let source_content = fs::read_to_string(&tx_config_file)?;
    TxConfig::from_str(&source_content)
}

impl TxConfig {
    pub fn from_str(content: &str) -> Result<Self, TxConfigLoadError> {
        let mut config = Ini::new();
        config.read(content.to_string())
            .map_err(|err| TxConfigLoadError::ParseError(err.to_string()))?;
        let mut main_section = TxConfigSectionMain::default();
        main_section.host = config.get("main", "host").unwrap_or("https://www.transifex.com".to_string());
        main_section.minimum_prec = config.getint("main", "minimum_perc").unwrap_or(None);
        main_section.mode = config.get("main", "mode");

        let mut tx_config = TxConfig {
            main_section,
            ..TxConfig::default()
        };

        let sections = config.sections();
        for section in sections {
            if section == "main" {
                continue;
            }
            let resource_section = TxConfigSectionResource {
                resource_full_slug: section.to_string(),
                file_filter: config.get(&section, "file_filter").ok_or(TxConfigLoadError::ParseError("missing file_filter key".to_string()))?,
                minimum_prec: config.getint(&section, "minimum_perc").unwrap_or(None),
                source_file: config.get(&section, "source_file").ok_or(TxConfigLoadError::ParseError("missing source_file key".to_string()))?,
                source_lang: config.get(&section, "source_lang").ok_or(TxConfigLoadError::ParseError("missing source_lang key".to_string()))?,
                type_attr: config.get(&section, "type").ok_or(TxConfigLoadError::ParseError("missing type key".to_string()))?,
            };
            tx_config.resource_sections.push(resource_section);
        };
        Ok(tx_config)
    }

    pub fn to_str(&self) -> String {
        let mut config = Ini::new();
        config.setstr("main", "host", Some(&self.main_section.host));
        if let Some(minimum_prec) = self.main_section.minimum_prec {
            config.setstr("main", "minimum_perc", Some(&minimum_prec.to_string()));
        };
        if let Some(mode) = &self.main_section.mode {
            config.setstr("main", "mode", Some(&mode));
        };

        for resource_section in &self.resource_sections {
            config.setstr(&resource_section.resource_full_slug, "file_filter", Some(&resource_section.file_filter));
            if let Some(minimum_prec) = resource_section.minimum_prec {
                config.setstr(&resource_section.resource_full_slug, "minimum_perc", Some(&minimum_prec.to_string()));
            };
            config.setstr(&resource_section.resource_full_slug, "source_file", Some(&resource_section.source_file));
            config.setstr(&resource_section.resource_full_slug, "source_lang", Some(&resource_section.source_lang));
            config.setstr(&resource_section.resource_full_slug, "type", Some(&resource_section.type_attr));
        }

        let mut write_options = WriteOptions::default();
        write_options.space_around_delimiters = true;
        write_options.blank_lines_between_sections = 1;
        config.pretty_writes(&write_options)
    }

    pub fn to_transifex_yaml(&self) -> TransifexYaml {
        let mut filters = Vec::<transifex_yaml_file::Filter>::new();
        for resource_section in &self.resource_sections {
            let filter = transifex_yaml_file::Filter {
                type_attr: "file".to_string(),
                source: resource_section.source_file.clone(),
                format: resource_section.type_attr.clone(),
                source_lang: resource_section.source_lang.clone(),
                target_pattern: resource_section.file_filter.clone(),
            };
            filters.push(filter);
        };
        TransifexYaml {
            filters,
            settings: transifex_yaml_file::Settings {
                branch_template: "transifex_update_<br_unique_id>".to_string()
            }
        }
    }
}

#[derive(Default)]
pub struct TxConfigSectionMain {
    pub host: String,
    pub minimum_prec: Option<i64>,
    pub mode: Option<String>,
}

#[derive(Default)]
pub struct TxConfigSectionResource {
    pub resource_full_slug: String,
    pub file_filter: String,
    pub minimum_prec: Option<i64>,
    pub source_file: String,
    pub source_lang: String,
    pub type_attr: String,
}

impl TxConfigSectionResource {
    #[cfg(test)]
    pub fn get_opr_slugs(&self) -> Result<(String, String, String), TxConfigLoadError> {
        // regex match section name, and extract organization_slug, project_slug, resource_slug.
        // section name format: o:organization_slug:p:project_slug:r:resource_slug
        let re = regex::Regex::new(r"o:(?P<organization_slug>[^:]+):p:(?P<project_slug>[^:]+):r:(?P<resource_slug>[^:]+)").unwrap();
        let caps = re.captures(&self.resource_full_slug).ok_or(TxConfigLoadError::ParseError("Invalid section name".to_string()))?;
        let organization_slug = caps.name("organization_slug").unwrap().as_str();
        let project_slug = caps.name("project_slug").unwrap().as_str();
        let resource_slug = caps.name("resource_slug").unwrap().as_str();
        Ok((organization_slug.to_string(), project_slug.to_string(), resource_slug.to_string()))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const TEST_TRANSIFEXRC_CONTENT: &str = r#"[https://www.transifex.com]
rest_hostname = https://rest.api.transifex.com
api_hostname = https://api.transifex.com
hostname = https://www.transifex.com
token = 1/23456789abcdef0123456789abcdef
"#;

    pub const TEST_TX_CONFIG_CONTENT: &str = r#"[main]
host = https://www.transifex.com
minimum_perc = 80
mode = developer

[o:linuxdeepin:p:deepin-desktop-environment:r:dde-control-center]
file_filter = translations/dde-control-center_<lang>.ts
minimum_perc = 0
source_file = translations/dde-control-center_en.ts
source_lang = en
type = QT

[o:linuxdeepin:p:deepin-desktop-environment:r:dde-control-center-desktop]
file_filter = translations/desktop/desktop_<lang>.ts
source_file = translations/desktop/desktop.ts
source_lang = en
type = QT
"#;

    #[test]
    fn tst_parse_transifexrc_content() {
        let transifexrc = TransifexRcSection::from_str(TEST_TRANSIFEXRC_CONTENT).unwrap();
        assert_eq!(transifexrc.host_section, "https://www.transifex.com");
        assert_eq!(transifexrc.rest_hostname, "https://rest.api.transifex.com");
        assert_eq!(transifexrc.token, "1/23456789abcdef0123456789abcdef");
    }

    #[test]
    fn tst_parse_tx_config_content() {
        let tx_config = TxConfig::from_str(TEST_TX_CONFIG_CONTENT).unwrap();
        assert_eq!(tx_config.main_section.host, "https://www.transifex.com");
        assert_eq!(tx_config.main_section.minimum_prec, Some(80));
        assert_eq!(tx_config.main_section.mode, Some("developer".to_string()));
        assert_eq!(tx_config.resource_sections.len(), 2);
        let (organization_slug, project_slug, resource_slug) = tx_config.resource_sections[0].get_opr_slugs().unwrap();
        assert_eq!(organization_slug, "linuxdeepin");
        assert_eq!(project_slug, "deepin-desktop-environment");
        assert_eq!(resource_slug, "dde-control-center");
        assert_eq!(tx_config.resource_sections[0].file_filter, "translations/dde-control-center_<lang>.ts");
        assert_eq!(tx_config.resource_sections[0].minimum_prec, Some(0));
        assert_eq!(tx_config.resource_sections[0].source_file, "translations/dde-control-center_en.ts");
        assert_eq!(tx_config.resource_sections[0].source_lang, "en");
        assert_eq!(tx_config.resource_sections[0].type_attr, "QT");
        let (organization_slug, project_slug, resource_slug) = tx_config.resource_sections[1].get_opr_slugs().unwrap();
        assert_eq!(organization_slug, "linuxdeepin");
        assert_eq!(project_slug, "deepin-desktop-environment");
        assert_eq!(resource_slug, "dde-control-center-desktop");
        assert_eq!(tx_config.resource_sections[1].file_filter, "translations/desktop/desktop_<lang>.ts");
        assert_eq!(tx_config.resource_sections[1].minimum_prec, None);
        assert_eq!(tx_config.resource_sections[1].source_file, "translations/desktop/desktop.ts");
        assert_eq!(tx_config.resource_sections[1].source_lang, "en");
        let content = tx_config.to_str();
        assert_eq!(content, TEST_TX_CONFIG_CONTENT);
    }
}

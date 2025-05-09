// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use std::{fs, path::PathBuf};

use crate::transifex::tx_config_file::{TxConfig, TxConfigSectionMain, TxConfigSectionResource};

use super::yaml2txconfig::create_linked_resources_table;

pub fn subcmd_monotxconfig(project_root: &PathBuf, force_online: bool, organization_slug: String) {
    let linked_resources = create_linked_resources_table(&organization_slug, None, force_online);

    let mut resource_sections = Vec::<TxConfigSectionResource>::new();

    for resource in linked_resources {
        let mut resource_section = TxConfigSectionResource::default();
        let source_file = resource.resource;
        resource_section.source_file = format!("{repository}/{source_file}", repository = resource.repository);
        resource_section.source_lang = "en_US".to_owned();
        resource_section.type_attr = if source_file.ends_with(".po") { "PO" } else { "QT" }.to_owned();

        let mut target_file = source_file.clone();
        if target_file.contains("_en_US") {
            target_file = target_file.replace("_en_US", "_<lang>");
        } else if target_file.contains("_en") {
            target_file = target_file.replace("_en", "_<lang>");
        } else {
            if let Some((name, ext)) = target_file.rsplit_once('.') {
                target_file = format!("{}_{}.{}", name, "<lang>", ext);
            }
        }
        resource_section.file_filter = format!("{repository}/{target_file}", repository = resource.repository);
        resource_section.resource_full_slug = resource.transifex_resource_id;

        resource_sections.push(resource_section);
    }

    let txconfig_file = TxConfig {
        main_section: TxConfigSectionMain {
            host: "https://www.transifex.com".to_string(),
            ..TxConfigSectionMain::default()
        },
        resource_sections,
    };

    let tx_config_file = project_root.join(".tx/config");
    if tx_config_file.exists() {
        println!("Note: {tx_config_file:?} file already exists, not overwriting it.");
        println!("You can use the following context to update the file manually:\n");
        println!("{}", txconfig_file.to_str());
    } else {
        let parent_dir = tx_config_file.parent().unwrap();
        fs::create_dir_all(&parent_dir).expect("Failed to create .tx directory");
        fs::write(&tx_config_file, txconfig_file.to_str()).expect("Failed to write .tx/config file");
        println!("Generated .tx/config file at: {tx_config_file:?}");
    }
}
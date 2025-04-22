// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use std::{fs, path::PathBuf};
use quick_xml::Writer;
use zhconv::zhconv;

use crate::linguist_file::*;

pub fn subcmd_zhconv(source_language: String, target_languages: Vec<String>, linguist_ts_file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if !linguist_ts_file.is_file() {
        println!("Provided file {} does not exist", linguist_ts_file.display());
        return Err("Provided file does not exist".into());
    }
    let file_name = linguist_ts_file.file_name().ok_or("Failed to get file name").unwrap();
    if !file_name.to_string_lossy().contains(&source_language) {
        println!("Input file {linguist_ts_file:?} doesn't have the source language {source_language:?} in its file name.");
        return Err("Input file doesn't match to the source language".into());
    }

    let source_content = match load_ts_file(&linguist_ts_file) {
        Ok(source_content) => source_content,
        Err(_e) => {
            println!("Failed to load source file: {file_name:?}");
            return Err("Failed to load source file".into());
        }
    };

    let mut target_contents: Vec<(PathBuf, Ts)> = vec![];
    for target_language in target_languages {
        // replace the source language code with the target language code to get the target file name
        let target_file_name = file_name.to_string_lossy().replace(&source_language, &target_language);
        let target_file_path = linguist_ts_file.parent().unwrap().join(&target_file_name);
        let target_content = match load_ts_file_or_default(&target_file_path, &source_content, &target_language) {
            Ok(target_content) => target_content,
            Err(_e) => {
                println!("Failed to load target file: {target_file_name:?}");
                return Err("Failed to load target file".into());
            }
        };
        target_contents.push((target_file_path, target_content));
    }

    for (target_path, target_content) in &mut target_contents {
        if target_content.contexts.len() != source_content.contexts.len() {
            println!("Target file {} has different number of contexts", target_content.language);
            return Err("Target file has different number of contexts".into());
        }
        for (index, context) in target_content.contexts.iter_mut().enumerate() {
            let source_context = &source_content.contexts[index];
            if context.messages.len() != source_context.messages.len() {
                println!("Target file {} has different number of messages", target_content.language);
                return Err("Target file has different number of messages".into());
            }
            // for loop with index so we could access the source context and message at the same index
            for (index, message) in context.messages.iter_mut().enumerate() {
                let source_message = &source_context.messages[index];
                // Skip the message if it's finished
                if !matches!(message.translation.type_attr, Some(ref s) if s == "unfinished") {
                    continue;
                }
                if matches!(source_message.translation.type_attr, Some(ref s) if s == "unfinished") {
                    continue;
                }
                if source_message.translation.value.is_some() {
                    message.translation.value = Some(zhconv(source_message.translation.value.as_ref().unwrap(), correct_language_code(&target_content.language).parse().unwrap()));
                    message.translation.type_attr = None;
                }
            }
        }

        let target_file = fs::File::create(target_path)?;
        let mut writer = Writer::new_with_indent(&target_file, b' ', 4);
        writer.write_linguist_ts_file(&target_content).unwrap();
    }

    Ok(())
}

pub fn subcmd_zhconv_plain(target_languages: Vec<String>, content: String) -> Result<(), Box<dyn std::error::Error>> {
    for target_language in target_languages {
        let converted = zhconv(content.as_str(), correct_language_code(&target_language).parse().unwrap());
        println!("{}", converted);
    }

    Ok(())
}
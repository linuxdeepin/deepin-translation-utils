// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use thiserror::Error as TeError;
use std::path::PathBuf;
use zhconv::zhconv;

use crate::linguist_file::*;

#[derive(TeError, Debug)]
pub enum CmdError {
    #[error("Provided file {0:?} does not exist")]
    FileNotFound(PathBuf),
    #[error("Failed to get file name")]
    NoFileName,
    #[error("Failed to get directory name")]
    NoDirName,
    #[error("Input file {0:?} doesn't have the source language {1:?} in its file name.")]
    MismatchedLanguage(PathBuf, String),
    #[error("Fail to load source file {0:?} because: {1}")]
    LoadSourceFile(PathBuf, #[source] TsLoadError),
    #[error("Fail to load target file {0:?} because: {1}")]
    LoadTargetFile(PathBuf, #[source] TsLoadError),
    #[error("Target file {0:?} has different number of contexts")]
    DifferentContexts(String),
    #[error("Target file {0:?} has different number of messages")]
    DifferentMessages(String),
    #[error("Fail to save file {0:?} because: {1}")]
    SaveFile(PathBuf, #[source] TsSaveError),
    #[error("Fail to parse language code")]
    ParseLanguageCode,
}

fn zhconv_wrapper(text: &String, target: &String) -> Result<String, CmdError> {
    let target = correct_language_code(target);
    let target = target.parse().or_else(|_err| { Err(CmdError::ParseLanguageCode) })?;
    Ok(zhconv(text.as_str(), target))
}

fn translate_ts_content(source_content: &Ts, target_content: &mut Ts) -> Result<(), CmdError> {
    if target_content.contexts.len() != source_content.contexts.len() {
        return Err(CmdError::DifferentContexts(target_content.language.clone()));
    }
    for (index, context) in target_content.contexts.iter_mut().enumerate() {
        let source_context = &source_content.contexts[index];
        if context.messages.len() != source_context.messages.len() {
            return Err(CmdError::DifferentMessages(target_content.language.clone()));
        }
        // for loop with index so we could access the source context and message at the same index
        for (index, message) in context.messages.iter_mut().enumerate() {
            let source_message = &source_context.messages[index];
            // Skip the message if it's finished
            if !matches!(message.translation.type_attr, Some(TranslationType::Unfinished)) {
                continue;
            }
            if matches!(source_message.translation.type_attr, Some(TranslationType::Unfinished)) {
                continue;
            }
            if let Some(value) = &source_message.translation.value {
                message.fill_translation(&zhconv_wrapper(&value, &target_content.language)?);
            }
        }
    }
    Ok(())
}

pub fn subcmd_zhconv(source_language: String, target_languages: Vec<String>, linguist_ts_file: PathBuf) -> Result<(), CmdError> {
    if !linguist_ts_file.is_file() {
        return Err(CmdError::FileNotFound(linguist_ts_file.clone()));
    }
    let file_name = linguist_ts_file.file_name().ok_or(CmdError::NoFileName)?;
    if !file_name.to_string_lossy().contains(&source_language) {
        return Err(CmdError::MismatchedLanguage(linguist_ts_file.clone(), source_language.clone()));
    }

    let source_content = load_ts_file(&linguist_ts_file)
        .or_else(|e| { Err(CmdError::LoadSourceFile(linguist_ts_file.clone(), e)) })?;

    let mut target_contents: Vec<(PathBuf, Ts)> = vec![];
    for target_language in target_languages {
        // replace the source language code with the target language code to get the target file name
        let target_file_name = file_name.to_string_lossy().replace(&source_language, &target_language);
        let target_file_path = linguist_ts_file.parent().ok_or(CmdError::NoDirName)
            .and_then(|p| { Ok(p.join(&target_file_name)) })?;
        let target_content = load_ts_file_or_default(&target_file_path, &source_content, &target_language)
            .or_else(|e| { Err(CmdError::LoadTargetFile(target_file_path.clone(), e)) })?;
        target_contents.push((target_file_path, target_content));
    }

    for (target_path, target_content) in &mut target_contents {
        translate_ts_content(&source_content, target_content)?;

        save_ts_file(&target_path, &target_content)
            .or_else(|err| { Err(CmdError::SaveFile(target_path.clone(), err)) })?;
    }

    Ok(())
}

pub fn subcmd_zhconv_plain(target_languages: Vec<String>, content: String) -> Result<(), CmdError> {
    for target_language in target_languages {
        let converted = zhconv_wrapper(&content, &target_language)?;
        println!("{}", converted);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linguist_file::tests::*;

    #[test]
    fn tst_translate_ts_content() {
        let source_ts: Ts = quick_xml::de::from_str(TEST_ZH_CN_TS_CONTENT).unwrap();
        let mut target_ts: Ts = source_ts.clone();
        target_ts.language = "zh_TW".to_string();
        target_ts.clear_finished_messages();
        assert!(translate_ts_content(&source_ts, &mut target_ts).is_ok());
        assert_eq!(target_ts.language, "zh_TW");
        assert_eq!(target_ts.contexts.len(), 1);
        assert_eq!(target_ts.contexts[0].messages.len(), 4);
        assert_eq!(target_ts.contexts[0].messages[0].translation.value, Some(String::from("海內存知己")));
        assert_eq!(target_ts.contexts[0].messages[1].translation.value, Some(String::from("軟體開發工程師在使用滑鼠操作螢幕上的游標")));
        assert_eq!(target_ts.contexts[0].messages[2].translation.value, Some(String::from("电视频段"))); // marked as obsolete, should not be translated.
        assert_eq!(target_ts.contexts[0].messages[3].translation.value, None); // source is also untranslated
    }
}

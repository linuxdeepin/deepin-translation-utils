// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use thiserror::Error as TeError;
use std::path::{Path, PathBuf};
use zhconv::zhconv;
use crate::i18n_file::{self, linguist::Ts, gettext::Po};

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
    #[error("Target file {0:?} has different number of contexts")]
    DifferentContexts(String),
    #[error("Target file for language {0:?} has different number of messages (Source {1:?} != Target {2:?})")]
    DifferentMessages(String, usize, usize),
    #[error("Fail to parse language code")]
    ParseLanguageCode,
    #[error("Missing language code in Linguist TS file")]
    MissingLanguageCode,
    #[error("Can not guess translation file kind from path {0:?} because: {1}")]
    GuessI18nFileType(PathBuf, #[source] i18n_file::common::UnknownI18nFileExtError),
    #[error("The translation file type of target file and reference file is mismatched.")]
    MismatchedI18nFileType,
    #[error("Fail to load source file {0:?} because: {1}")]
    LoadTsSourceFile(PathBuf, #[source] i18n_file::linguist::TsLoadError),
    #[error("Fail to load source file {0:?} because: {1}")]
    LoadPoSourceFile(PathBuf, #[source] i18n_file::gettext::PoLoadError),
    #[error("Fail to load target file {0:?} because: {1}")]
    LoadTsTargetFile(PathBuf, #[source] i18n_file::linguist::TsLoadError),
    #[error("Fail to load target file {0:?} because: {1}")]
    LoadPoTargetFile(PathBuf, #[source] i18n_file::gettext::PoLoadError),
    #[error("Fail to save file {0:?} because: {1}")]
    SaveTsFile(PathBuf, #[source] i18n_file::linguist::TsSaveError),
    #[error("Fail to save file {0:?} because: {1}")]
    SavePoFile(PathBuf, #[source] i18n_file::gettext::PoSaveError),
}

// ===== Utils Functions =====

fn correct_language_code(language_code: &str) -> String {
    return language_code.replace("_", "-");
}

fn zhconv_wrapper(text: &str, target: &str) -> Result<String, CmdError> {
    let target = correct_language_code(target);
    let target = target.parse().map_err(|_| CmdError::ParseLanguageCode)?;
    Ok(zhconv(text, target))
}

fn translate_ts_content(source_content: &Ts, target_content: &mut Ts) -> Result<(), CmdError> {
    use i18n_file::linguist::TranslationType;

    let language_code = target_content.get_language().ok_or(CmdError::MissingLanguageCode)?;
    if target_content.contexts.len() != source_content.contexts.len() {
        return Err(CmdError::DifferentContexts(language_code.clone()));
    }
    for (index, context) in target_content.contexts.iter_mut().enumerate() {
        let source_context = &source_content.contexts[index];
        if context.messages.len() != source_context.messages.len() {
            return Err(CmdError::DifferentMessages(language_code.clone(), source_context.messages.len(), context.messages.len()));
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
                message.fill_translation(&zhconv_wrapper(&value, &language_code)?);
            }
        }
    }
    Ok(())
}

fn translate_po_content(source_content: &Po, target_content: &mut Po) -> Result<(), CmdError> {
    use polib::message::{MessageMutView, MessageView};

    let language_code = target_content.get_language();
    let source_catalog = &source_content.inner;
    let target_catalog = &mut target_content.inner;

    let target_msg_count = target_catalog.count();
    let source_msg_count = source_catalog.count();
    if target_msg_count != source_msg_count {
        return Err(CmdError::DifferentMessages(language_code, source_msg_count, target_msg_count));
    };
    for (mut message, reference_message) in target_catalog.messages_mut().zip(source_catalog.messages()) {
        if message.is_translated() {
            continue;
        };
        if reference_message.is_translated() && !message.is_translated() && !message.is_plural() {
            // We have checked plural case, unwrap directly.
            let msgstr = reference_message.msgstr().unwrap().to_string();
            let translated_msg = zhconv_wrapper(&msgstr, &language_code)?;
            message.set_msgstr(translated_msg).unwrap();
        };
    }
    Ok(())
}

// ===== Uniform Translation File =====

enum ZhConvFile {
    Linguist(Ts),
    Gettext(Po),
}
impl ZhConvFile {
    fn load_file(file_path: &Path) -> Result<Self, CmdError> {
        use i18n_file::common::I18nFileKind;
        // Detect translation file kind from given file extension.
        let i18n_file_kind = I18nFileKind::from_ext_hint(file_path)
            .map_err(|e| CmdError::GuessI18nFileType(file_path.to_path_buf(), e))?;
        // Dispatch loading request.
        Ok(match i18n_file_kind {
            I18nFileKind::Linguist => Self::Linguist(
                Ts::load_from_file(file_path)
                    .map_err(|e| CmdError::LoadTsSourceFile(file_path.to_path_buf(), e))?,
            ),
            I18nFileKind::Gettext => Self::Gettext(
                Po::load_from_file(file_path)
                    .map_err(|e| CmdError::LoadPoSourceFile(file_path.to_path_buf(), e))?,
            ),
        })
    }

    fn load_or_create_target_file(&self, file_path: &Path, fallback_language_code: &str) -> Result<Self, CmdError> {
        Ok(match self {
            ZhConvFile::Linguist(ts) => Self::Linguist(
                Ts::load_from_file_or_default(file_path, ts, fallback_language_code)
                    .map_err(|e| CmdError::LoadTsTargetFile(file_path.to_path_buf(), e))?,
            ),
            ZhConvFile::Gettext(po) => Self::Gettext(
                Po::load_from_file_or_default(file_path, po, fallback_language_code)
                    .map_err(|e| CmdError::LoadPoTargetFile(file_path.to_path_buf(), e))?,
            ),
        })
    }

    fn get_language(&self) -> Option<String> {
        match self {
            ZhConvFile::Linguist(ts) => ts.get_language(),
            ZhConvFile::Gettext(po) => Some(po.get_language()),
        }
    }

    fn set_language(&mut self, language_code: &str) {
        match self {
            ZhConvFile::Linguist(ts) => ts.set_language(language_code),
            ZhConvFile::Gettext(po) => po.set_language(language_code),
        }
    }
    
    fn translate_content_based_on(&mut self, reference_content: &Self) -> Result<(), CmdError> {
        match (self, reference_content) {
            (ZhConvFile::Linguist(lhs), ZhConvFile::Linguist(rhs)) => {
                Ok(translate_ts_content(rhs, lhs)?)
            },
            (ZhConvFile::Gettext(lhs), ZhConvFile::Gettext(rhs)) => {
                Ok(translate_po_content(rhs, lhs)?)
            },
            _ => Err(CmdError::MismatchedI18nFileType)
        }
    }

    fn save_file(&self, file_path: &Path) -> Result<(), CmdError> {
        Ok(match self {
            ZhConvFile::Linguist(ts) => ts
                .save_into_file(file_path)
                .map_err(|e| CmdError::SaveTsFile(file_path.to_path_buf(), e))?,
            ZhConvFile::Gettext(po) => po
                .save_into_file(file_path)
                .map_err(|e| CmdError::SavePoFile(file_path.to_path_buf(), e))?,
        })
    }
}

// ===== Sub Command =====

pub fn subcmd_zhconv(source_language: &str, target_languages: &[String], linguist_ts_file: &Path) -> Result<(), CmdError> {
    if !linguist_ts_file.is_file() {
        return Err(CmdError::FileNotFound(linguist_ts_file.to_path_buf()));
    }
    let file_name = linguist_ts_file.file_name().ok_or(CmdError::NoFileName)?;
    if !file_name.to_string_lossy().contains(&source_language) {
        return Err(CmdError::MismatchedLanguage(linguist_ts_file.to_path_buf(), source_language.to_string()));
    }

    let source_content = ZhConvFile::load_file(linguist_ts_file)?;

    let mut target_contents: Vec<(PathBuf, ZhConvFile)> = vec![];
    for target_language in target_languages {
        // replace the source language code with the target language code to get the target file name
        let target_file_name = file_name.to_string_lossy().replace(source_language, &target_language);
        let target_file_path = linguist_ts_file.parent().ok_or(CmdError::NoDirName)
            .and_then(|p| { Ok(p.join(target_file_name)) })?;
        let mut target_content = source_content.load_or_create_target_file(&target_file_path, &target_language)?;
        // if the target file's language code is not match to target_language, set it to target_language
        if !matches!(&target_content.get_language(), Some(lang) if lang == target_language.as_str()) {
            eprintln!("Warning: Target file {target_file_path:?} has no or unmatched language code, will set it to {target_language}.");
            target_content.set_language(&target_language);
        }
        target_contents.push((target_file_path, target_content));
    }

    for (target_path, target_content) in &mut target_contents {
        target_content.translate_content_based_on(&source_content)?;
        target_content.save_file(target_path)?;
    }

    Ok(())
}

pub fn subcmd_zhconv_plain(target_languages: &[String], content: &str) -> Result<(), CmdError> {
    for target_language in target_languages {
        let converted = zhconv_wrapper(&content, &target_language)?;
        println!("{}", converted);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tst_translate_ts_content() {
        use crate::i18n_file::linguist::Ts;
        use crate::i18n_file::linguist::tests::TEST_ZH_CN_TS_CONTENT;

        let source_ts: Ts = Ts::load_from_from_str(TEST_ZH_CN_TS_CONTENT).unwrap();
        let mut target_ts: Ts = source_ts.clone();
        target_ts.set_language("zh_TW");
        target_ts.clear_finished_messages();
        assert!(translate_ts_content(&source_ts, &mut target_ts).is_ok());
        assert_eq!(target_ts.get_language(), Some("zh_TW".to_string()));
        assert_eq!(target_ts.contexts.len(), 1);
        assert_eq!(target_ts.contexts[0].messages.len(), 5);
        assert_eq!(target_ts.contexts[0].messages[0].translation.value, Some(String::from("海內存知己")));
        assert_eq!(target_ts.contexts[0].messages[1].translation.value, Some(String::from("軟體開發工程師在使用滑鼠操作螢幕上的游標")));
        assert_eq!(target_ts.contexts[0].messages[2].translation.value, Some(String::from("电视频段"))); // marked as obsolete, should not be translated.
        assert_eq!(target_ts.contexts[0].messages[3].translation.value, None); // source is also untranslated
    }

    #[test]
    fn tst_translate_po_content() {
        use crate::i18n_file::gettext::Po;
        use crate::i18n_file::gettext::tests::TEST_ZH_CN_PO_CONTENT;

        let source_po = Po::load_from_str(TEST_ZH_CN_PO_CONTENT).unwrap();
        let mut target_po = source_po.clone();
        target_po.set_language("zh_TW");
        target_po.clear_finished_messages();
        assert!(translate_po_content(&source_po, &mut target_po).is_ok());
        assert_eq!(target_po.get_language(), "zh_TW".to_string());
        assert_eq!(target_po.inner.count(), 4);
        let mut msgs = target_po.inner.messages();
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "海內存知己");
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "軟體開發工程師在使用滑鼠操作螢幕上的游標");
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), ""); // marked as obsolete. but polib will not read it.
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), ""); // source is also untranslated
    }
}

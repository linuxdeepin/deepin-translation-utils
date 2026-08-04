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
    #[error("Target language ({0:?})'s source string doesn't match (Source: {1:?} != Target: {2:?}), did you forget to run `update_translations` beforehand?")]
    DifferentMessage(String, String, String),
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

fn is_two_chinese_chars(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() != 2 {
        return false;
    }
    chars.iter().all(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
}

fn strip_spaces(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn needs_button_spacing(value: &str, has_button_comment: bool) -> bool {
    if !has_button_comment {
        return false;
    }
    let stripped = strip_spaces(value);
    is_two_chinese_chars(&stripped)
}

fn process_button_translation(
    value: &str,
    has_button_comment: bool,
    enable_button_spacing: bool,
    target_language: &str,
) -> Result<String, CmdError> {
    if !enable_button_spacing || !needs_button_spacing(value, has_button_comment) {
        return zhconv_wrapper(value, target_language);
    }
    let stripped = strip_spaces(value);
    let converted = zhconv_wrapper(&stripped, target_language)?;
    let chars: Vec<char> = converted.chars().collect();
    Ok(format!("{} {}", chars[0], chars[1]))
}

fn apply_button_spacing_to_value(value: &str, has_button_comment: bool, enable: bool) -> Option<String> {
    if !enable || !needs_button_spacing(value, has_button_comment) {
        return None;
    }
    let stripped = strip_spaces(value);
    let chars: Vec<char> = stripped.chars().collect();
    Some(format!("{} {}", chars[0], chars[1]))
}

fn apply_ts_source_button_spacing(source: &mut Ts, enable: bool) {
    for context in &mut source.contexts {
        for message in &mut context.messages {
            let has_button = message.comment.as_ref()
                .map(|c| c == "button")
                .unwrap_or(false);
            if let Some(value) = &message.translation.value {
                if let Some(spaced) = apply_button_spacing_to_value(value, has_button, enable) {
                    message.translation.value = Some(spaced);
                }
            }
        }
    }
}

fn apply_po_source_button_spacing(source: &mut Po, enable: bool) {
    use polib::message::{MessageMutView, MessageView};
    for mut message in source.inner.messages_mut() {
        let has_button = message.extracted_comments().split_whitespace().any(|w| w == "button")
            || message.translator_comments().split_whitespace().any(|w| w == "button");
        if let Ok(msgstr) = message.msgstr() {
            let msgstr = msgstr.to_string();
            if let Some(spaced) = apply_button_spacing_to_value(&msgstr, has_button, enable) {
                let _ = message.set_msgstr(spaced);
            }
        }
    }
}

fn translate_ts_content(source_content: &Ts, target_content: &mut Ts, enable_button_spacing: bool) -> Result<(), CmdError> {
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
            if source_message.source != message.source {
                return Err(CmdError::DifferentMessage(language_code.clone(), source_message.source.clone(), message.source.clone()));
            }
            if let Some(value) = &source_message.translation.value {
                let has_button_comment = source_message.comment.as_ref()
                    .map(|c| c == "button")
                    .unwrap_or(false);
                let processed_value = process_button_translation(
                    value,
                    has_button_comment,
                    enable_button_spacing,
                    &language_code,
                )?;
                message.fill_translation(&processed_value);
            }
        }
    }
    Ok(())
}

fn translate_po_content(source_content: &Po, target_content: &mut Po, enable_button_spacing: bool) -> Result<(), CmdError> {
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
            let has_button_comment =
                reference_message.extracted_comments().split_whitespace().any(|w| w == "button")
                || reference_message.translator_comments().split_whitespace().any(|w| w == "button");
            let translated_msg = process_button_translation(&msgstr, has_button_comment, enable_button_spacing, &language_code)?;
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
    
    fn apply_source_button_spacing(&mut self, enable: bool) {
        match self {
            ZhConvFile::Linguist(ts) => apply_ts_source_button_spacing(ts, enable),
            ZhConvFile::Gettext(po) => apply_po_source_button_spacing(po, enable),
        }
    }

    fn translate_content_based_on(&mut self, reference_content: &Self, enable_button_spacing: bool) -> Result<(), CmdError> {
        match (self, reference_content) {
            (ZhConvFile::Linguist(lhs), ZhConvFile::Linguist(rhs)) => {
                Ok(translate_ts_content(rhs, lhs, enable_button_spacing)?)
            },
            (ZhConvFile::Gettext(lhs), ZhConvFile::Gettext(rhs)) => {
                Ok(translate_po_content(rhs, lhs, enable_button_spacing)?)
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

pub fn subcmd_zhconv(source_language: &str, target_languages: &[String], linguist_ts_file: &Path, enable_button_spacing: bool) -> Result<(), CmdError> {
    if !linguist_ts_file.is_file() {
        return Err(CmdError::FileNotFound(linguist_ts_file.to_path_buf()));
    }
    let file_name = linguist_ts_file.file_name().ok_or(CmdError::NoFileName)?;
    if !file_name.to_string_lossy().contains(&source_language) {
        return Err(CmdError::MismatchedLanguage(linguist_ts_file.to_path_buf(), source_language.to_string()));
    }

    let mut source_content = ZhConvFile::load_file(linguist_ts_file)?;

    // Apply button spacing to source file's own translations
    source_content.apply_source_button_spacing(enable_button_spacing);
    source_content.save_file(linguist_ts_file)?;

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
        target_content.translate_content_based_on(&source_content, enable_button_spacing)?;
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

        let source_ts: Ts = Ts::load_from_str(TEST_ZH_CN_TS_CONTENT).unwrap();
        let mut target_ts: Ts = source_ts.clone();
        target_ts.set_language("zh_TW");
        target_ts.clear_finished_messages();
        assert!(translate_ts_content(&source_ts, &mut target_ts, true).is_ok());
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
        assert!(translate_po_content(&source_po, &mut target_po, true).is_ok());
        assert_eq!(target_po.get_language(), "zh_TW".to_string());
        assert_eq!(target_po.inner.count(), 4);
        let mut msgs = target_po.inner.messages();
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "海內存知己");
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "軟體開發工程師在使用滑鼠操作螢幕上的游標");
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), ""); // marked as obsolete. but polib will not read it.
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), ""); // source is also untranslated
    }

    #[test]
    fn tst_helper_is_two_chinese_chars() {
        assert!(is_two_chinese_chars("确定"));
        assert!(is_two_chinese_chars("取消"));
        assert!(is_two_chinese_chars("保存"));
        assert!(!is_two_chinese_chars("确定退出"));
        assert!(!is_two_chinese_chars("a"));
        assert!(!is_two_chinese_chars(""));
        assert!(!is_two_chinese_chars("abc"));
        assert!(!is_two_chinese_chars("OK"));
    }

    #[test]
    fn tst_helper_needs_button_spacing() {
        assert!(needs_button_spacing("确定", true));
        assert!(needs_button_spacing("确 定", true));
        assert!(needs_button_spacing("取消", true));
        assert!(needs_button_spacing("删 除", true));
        assert!(!needs_button_spacing("确定退出", true));
        assert!(!needs_button_spacing("确定", false));
        assert!(!needs_button_spacing("确定退出", false));
    }

    #[test]
    fn tst_translate_ts_content_with_button_spacing() {
        use crate::i18n_file::linguist::Ts;
        use crate::i18n_file::linguist::tests::TEST_ZH_CN_TS_CONTENT_WITH_BUTTON;

        let source_ts: Ts = Ts::load_from_str(TEST_ZH_CN_TS_CONTENT_WITH_BUTTON).unwrap();
        let mut target_ts: Ts = source_ts.clone();
        target_ts.set_language("zh_TW");
        target_ts.clear_finished_messages();
        assert!(translate_ts_content(&source_ts, &mut target_ts, true).is_ok());
        assert_eq!(target_ts.get_language(), Some("zh_TW".to_string()));
        assert_eq!(target_ts.contexts.len(), 1);
        assert_eq!(target_ts.contexts[0].messages.len(), 6);
        // message 0: "确定" (comment=button, 2 chars) -> should be "確 定"
        assert_eq!(target_ts.contexts[0].messages[0].translation.value, Some(String::from("確 定")));
        // message 1: "取消" (comment=button, 2 chars) -> should be "取 消"
        assert_eq!(target_ts.contexts[0].messages[1].translation.value, Some(String::from("取 消")));
        // message 2: "保存" (comment=button, 2 chars) -> should be "儲 存"
        assert_eq!(target_ts.contexts[0].messages[2].translation.value, Some(String::from("儲 存")));
        // message 3: "删 除" (comment=button, 2 chars, pre-spaced) -> strip space, convert, then re-space -> "刪 除"
        assert_eq!(target_ts.contexts[0].messages[3].translation.value, Some(String::from("刪 除")));
        // message 4: "确定退出" (comment=button, 4 chars) -> should NOT get spaced, just converted
        // message 5: "打开" (no comment, 2 chars) -> should NOT get spaced, just converted
        assert_eq!(target_ts.contexts[0].messages[5].translation.value, Some(String::from("打開")));
    }

    #[test]
    fn tst_translate_po_content_with_button_spacing() {
        use crate::i18n_file::gettext::Po;
        use crate::i18n_file::gettext::tests::TEST_ZH_CN_PO_CONTENT_WITH_BUTTON;

        let source_po = Po::load_from_str(TEST_ZH_CN_PO_CONTENT_WITH_BUTTON).unwrap();
        let mut target_po = source_po.clone();
        target_po.set_language("zh_TW");
        target_po.clear_finished_messages();
        assert!(translate_po_content(&source_po, &mut target_po, true).is_ok());
        assert_eq!(target_po.get_language(), "zh_TW".to_string());
        assert_eq!(target_po.inner.count(), 5);
        let mut msgs = target_po.inner.messages();
        // first msg: "确定" with #. button -> should be "確 定"
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "確 定");
        // second msg: "取消" with #. button -> should be "取 消"
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "取 消");
        // third msg: "确定退出" with #. button (4 chars) -> should NOT get spaced
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "確定退出");
        // fourth msg: "删 除" with #. button, pre-spaced 2 chars -> strip space, convert, re-space -> "刪 除"
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "刪 除");
        // fifth msg: "打开" without button comment -> should NOT get spaced
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "打開");
    }

    #[test]
    fn tst_translate_ts_content_with_button_spacing_disabled() {
        use crate::i18n_file::linguist::Ts;
        use crate::i18n_file::linguist::tests::TEST_ZH_CN_TS_CONTENT_WITH_BUTTON;

        let source_ts: Ts = Ts::load_from_str(TEST_ZH_CN_TS_CONTENT_WITH_BUTTON).unwrap();
        let mut target_ts: Ts = source_ts.clone();
        target_ts.set_language("zh_TW");
        target_ts.clear_finished_messages();
        assert!(translate_ts_content(&source_ts, &mut target_ts, false).is_ok());
        assert_eq!(target_ts.get_language(), Some("zh_TW".to_string()));
        assert_eq!(target_ts.contexts.len(), 1);
        assert_eq!(target_ts.contexts[0].messages.len(), 6);
        // all messages should be converted without button spacing
        assert_eq!(target_ts.contexts[0].messages[0].translation.value, Some(String::from("確定")));
        assert_eq!(target_ts.contexts[0].messages[1].translation.value, Some(String::from("取消")));
        assert_eq!(target_ts.contexts[0].messages[2].translation.value, Some(String::from("儲存")));
        assert_eq!(target_ts.contexts[0].messages[3].translation.value, Some(String::from("刪 除"))); // pre-existing space preserved by zhconv
        assert_eq!(target_ts.contexts[0].messages[4].translation.value, Some(String::from("確定退出")));
        assert_eq!(target_ts.contexts[0].messages[5].translation.value, Some(String::from("打開")));
    }

    #[test]
    fn tst_translate_po_content_with_button_spacing_disabled() {
        use crate::i18n_file::gettext::Po;
        use crate::i18n_file::gettext::tests::TEST_ZH_CN_PO_CONTENT_WITH_BUTTON;

        let source_po = Po::load_from_str(TEST_ZH_CN_PO_CONTENT_WITH_BUTTON).unwrap();
        let mut target_po = source_po.clone();
        target_po.set_language("zh_TW");
        target_po.clear_finished_messages();
        assert!(translate_po_content(&source_po, &mut target_po, false).is_ok());
        assert_eq!(target_po.get_language(), "zh_TW".to_string());
        assert_eq!(target_po.inner.count(), 5);
        let mut msgs = target_po.inner.messages();
        // all messages should be converted without button spacing
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "確定");
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "取消");
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "確定退出");
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "刪 除"); // pre-existing space preserved by zhconv
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "打開");
    }

    #[test]
    fn tst_apply_ts_source_button_spacing() {
        use crate::i18n_file::linguist::Ts;
        use crate::i18n_file::linguist::tests::TEST_ZH_CN_TS_CONTENT_WITH_BUTTON;

        let mut source_ts: Ts = Ts::load_from_str(TEST_ZH_CN_TS_CONTENT_WITH_BUTTON).unwrap();
        apply_ts_source_button_spacing(&mut source_ts, true);
        assert_eq!(source_ts.contexts[0].messages.len(), 6);
        // message 0: "确定" (comment=button, 2 chars) -> should become "确 定"
        assert_eq!(source_ts.contexts[0].messages[0].translation.value, Some(String::from("确 定")));
        // message 1: "取消" (comment=button, 2 chars) -> should become "取 消"
        assert_eq!(source_ts.contexts[0].messages[1].translation.value, Some(String::from("取 消")));
        // message 2: "保存" (comment=button, 2 chars) -> should become "保 存"
        assert_eq!(source_ts.contexts[0].messages[2].translation.value, Some(String::from("保 存")));
        // message 3: "删 除" (comment=button, 2 chars, pre-spaced) -> should stay "删 除"
        assert_eq!(source_ts.contexts[0].messages[3].translation.value, Some(String::from("删 除")));
        // message 4: "确定退出" (comment=button, 4 chars) -> should NOT change
        assert_eq!(source_ts.contexts[0].messages[4].translation.value, Some(String::from("确定退出")));
        // message 5: "打开" (no comment, 2 chars) -> should NOT change
        assert_eq!(source_ts.contexts[0].messages[5].translation.value, Some(String::from("打开")));
    }

    #[test]
    fn tst_apply_ts_source_button_spacing_disabled() {
        use crate::i18n_file::linguist::Ts;
        use crate::i18n_file::linguist::tests::TEST_ZH_CN_TS_CONTENT_WITH_BUTTON;

        let mut source_ts: Ts = Ts::load_from_str(TEST_ZH_CN_TS_CONTENT_WITH_BUTTON).unwrap();
        apply_ts_source_button_spacing(&mut source_ts, false);
        // no messages should be changed when disabled
        assert_eq!(source_ts.contexts[0].messages[0].translation.value, Some(String::from("确定")));
        assert_eq!(source_ts.contexts[0].messages[1].translation.value, Some(String::from("取消")));
        assert_eq!(source_ts.contexts[0].messages[2].translation.value, Some(String::from("保存")));
        assert_eq!(source_ts.contexts[0].messages[3].translation.value, Some(String::from("删 除")));
        assert_eq!(source_ts.contexts[0].messages[4].translation.value, Some(String::from("确定退出")));
        assert_eq!(source_ts.contexts[0].messages[5].translation.value, Some(String::from("打开")));
    }

    #[test]
    fn tst_apply_po_source_button_spacing() {
        use crate::i18n_file::gettext::Po;
        use crate::i18n_file::gettext::tests::TEST_ZH_CN_PO_CONTENT_WITH_BUTTON;

        let mut source_po = Po::load_from_str(TEST_ZH_CN_PO_CONTENT_WITH_BUTTON).unwrap();
        apply_po_source_button_spacing(&mut source_po, true);
        assert_eq!(source_po.inner.count(), 5);
        let mut msgs = source_po.inner.messages();
        // first msg: "确定" with #. button -> should become "确 定"
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "确 定");
        // second msg: "取消" with #. button -> should become "取 消"
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "取 消");
        // third msg: "确定退出" with #. button (4 chars) -> should NOT change
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "确定退出");
        // fourth msg: "删 除" with #. button, pre-spaced 2 chars -> should stay "删 除"
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "删 除");
        // fifth msg: "打开" without button comment -> should NOT change
        assert_eq!(msgs.next().unwrap().msgstr().unwrap(), "打开");
    }
}

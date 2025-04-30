// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use thiserror::Error as TeError;
use std::path::PathBuf;
use polib::{message::{MessageMutView, MessageView}, po_file::{self, POParseError}};
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
    #[error("Fail to load source file {0:?} because: {1}")]
    LoadPOSourceFile(PathBuf, #[source] POParseError),
    #[error("Fail to load target file {0:?} because: {1}")]
    LoadTargetFile(PathBuf, #[source] TsLoadError),
    #[error("Target file {0:?} has different number of contexts")]
    DifferentContexts(String),
    #[error("Target file for language {0:?} has different number of messages (Source {1:?} != Target {2:?})")]
    DifferentMessages(String, usize, usize),
    #[error("Fail to save file {0:?} because: {1}")]
    SaveFile(PathBuf, #[source] TsSaveError),
    #[error("Fail to save file {0:?} because: {1}")]
    SavePOFile(PathBuf, #[source] std::io::Error),
    #[error("Fail to parse language code")]
    ParseLanguageCode,
    #[error("Missing language code in Linguist TS file")]
    MissingLanguageCode,
}

fn zhconv_wrapper(text: &String, target: &String) -> Result<String, CmdError> {
    let target = correct_language_code(target);
    let target = target.parse().or_else(|_err| { Err(CmdError::ParseLanguageCode) })?;
    Ok(zhconv(text.as_str(), target))
}

trait ZhConvertible {
    type T;

    fn load_file(file_path: &PathBuf) -> Result<Self::T, CmdError>;
    fn load_or_create_target_file(&self, file_path: &PathBuf, fallback_language_code: &String) -> Result<Self::T, CmdError>;
    fn language(&self) -> Option<String>;
    fn set_language(&mut self, language_code: &String);
    fn translate_content_based_on(&mut self, reference_content: &Self) -> Result<(), CmdError>;
    fn save_file(&self, file_path: &PathBuf) -> Result<(), CmdError>;
}

enum ZhConvertibleType {
    Ts(Ts),
    Po(polib::catalog::Catalog),
}

impl ZhConvertible for ZhConvertibleType {
    type T = Self;
    
    fn load_file(file_path: &PathBuf) -> Result<Self::T, CmdError> {
        if !file_path.exists() {
            return Err(CmdError::FileNotFound(file_path.clone()));
        }
        if file_path.extension().and_then(|e| e.to_str()) == Some("ts") {
            Ts::load_file(file_path).map(Self::Ts)
        } else {
            polib::catalog::Catalog::load_file(file_path).map(Self::Po)
        }
    }
    
    fn load_or_create_target_file(&self, file_path: &PathBuf, fallback_language_code: &String) -> Result<Self::T, CmdError> {
        match self {
            Self::Ts(ts) => ts.load_or_create_target_file(file_path, fallback_language_code).map(Self::Ts),
            Self::Po(po) => po.load_or_create_target_file(file_path, fallback_language_code).map(Self::Po),
        }
    }

    fn language(&self) -> Option<String> {
        match self {
            Self::Ts(ts) => ts.language(),
            Self::Po(po) => po.language(),
        }
    }
    
    fn set_language(&mut self, language_code: &String) {
        match self {
            Self::Ts(ts) => ts.set_language(language_code),
            Self::Po(po) => po.set_language(language_code),
        }
    }
    
    fn translate_content_based_on(&mut self, reference_content: &Self) -> Result<(), CmdError> {
        match self {
            Self::Ts(ts) => {
                if let Self::Ts(reference_ts) = reference_content {
                    ts.translate_content_based_on(reference_ts)
                } else {
                    panic!("Unexpected reference content type")
                }
            }
            Self::Po(po) => {
                if let Self::Po(reference_po) = reference_content {
                    po.translate_content_based_on(reference_po)
                } else {
                    panic!("Unexpected reference content type")
                }
            }
        }
    }
    
    fn save_file(&self, file_path: &PathBuf) -> Result<(), CmdError> {
        match self {
            Self::Ts(ts) => ts.save_file(file_path),
            Self::Po(po) => po.save_file(file_path),
        }
    }
}

impl ZhConvertible for polib::catalog::Catalog {
    type T = Self;
    fn load_file(file_path: &PathBuf) -> Result<Self, CmdError> {
        po_file::parse(file_path).map_err(|e| {CmdError::LoadPOSourceFile(file_path.clone(), e)})
    }
    
    fn load_or_create_target_file(&self, file_path: &PathBuf, fallback_language_code: &String) -> Result<Self, CmdError> {
        if !file_path.exists() {
            let mut catalog = self.clone();
            catalog.set_language(fallback_language_code);
            for mut message in catalog.messages_mut() {
                if message.is_translated() && !message.is_plural() {
                    message.set_msgstr(String::new()).expect("Plural messages are not supported currently.")
                }
            }
            return Ok(catalog);
        } else {
            return Self::load_file(file_path);
        }
    }

    fn language(&self) -> Option<String> {
        Some(self.metadata.language.clone())
    }
    
    fn set_language(&mut self, language_code: &String) {
        self.metadata.language = language_code.clone();
    }
    
    fn translate_content_based_on(&mut self, reference_content: &Self::T) -> Result<(), CmdError> {
        if self.messages().count() != reference_content.messages().count() {
            return Err(CmdError::DifferentMessages(self.metadata.language.clone(), reference_content.messages().count(), self.messages().count()));
        };
        let language_code = self.metadata.language.clone();
        for (index, mut message) in self.messages_mut().enumerate() {
            let reference_message = reference_content.messages().nth(index).expect("Messages count mismatch");
            if message.is_translated() {
                continue;
            };
            if reference_message.is_translated() && !message.is_translated() && !message.is_plural() {
                let msgstr = reference_message.msgstr().expect("Plural messages are not supported currently.").to_string();
                let translated_msg = &zhconv_wrapper(&msgstr, &language_code)?;
                message.set_msgstr(translated_msg.clone()).expect("Plural messages are not supported currently.");
            };
        };
        Ok(())
    }

    fn save_file(&self, file_path: &PathBuf) -> Result<(), CmdError> {
        po_file::write_to_file(self, file_path).map_err(|e| {CmdError::SavePOFile(file_path.clone(), e)})
    }
}

impl ZhConvertible for Ts {
    type T = Self;
    fn load_file(file_path: &PathBuf) -> Result<Self, CmdError> {
        load_ts_file(file_path).or_else(|e| {
            Err(CmdError::LoadSourceFile(file_path.clone(), e))
        })
    }

    fn load_or_create_target_file(&self, file_path: &PathBuf, fallback_language_code: &String) -> Result<Ts, CmdError> {
        load_ts_file_or_default(file_path, &self, fallback_language_code).or_else(|e| {
            Err(CmdError::LoadTargetFile(file_path.clone(), e))
        })
    }

    fn language(&self) -> Option<String> {
        self.language.clone()
    }

    fn set_language(&mut self, language_code: &String) {
        self.set_language(language_code);
    }

    fn translate_content_based_on(&mut self, reference_content: &Self) -> Result<(), CmdError> {
        translate_ts_content(&reference_content, self)
    }

    fn save_file(&self, file_path: &PathBuf) -> Result<(), CmdError> {
        save_ts_file(file_path, &self).or_else(|e| {
            Err(CmdError::SaveFile(file_path.clone(), e))
        })
    }
}

fn translate_ts_content(source_content: &Ts, target_content: &mut Ts) -> Result<(), CmdError> {
    let language_code = target_content.language.as_ref().ok_or(CmdError::MissingLanguageCode)?;
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

pub fn subcmd_zhconv(source_language: String, target_languages: Vec<String>, linguist_ts_file: PathBuf) -> Result<(), CmdError> {
    if !linguist_ts_file.is_file() {
        return Err(CmdError::FileNotFound(linguist_ts_file.clone()));
    }
    let file_name = linguist_ts_file.file_name().ok_or(CmdError::NoFileName)?;
    if !file_name.to_string_lossy().contains(&source_language) {
        return Err(CmdError::MismatchedLanguage(linguist_ts_file.clone(), source_language.clone()));
    }

    let source_content = ZhConvertibleType::load_file(&linguist_ts_file)?;

    let mut target_contents: Vec<(PathBuf, ZhConvertibleType)> = vec![];
    for target_language in target_languages {
        // replace the source language code with the target language code to get the target file name
        let target_file_name = file_name.to_string_lossy().replace(&source_language, &target_language);
        let target_file_path = linguist_ts_file.parent().ok_or(CmdError::NoDirName)
            .and_then(|p| { Ok(p.join(&target_file_name)) })?;
        let mut target_content = source_content.load_or_create_target_file(&target_file_path, &target_language)?;
        // if the target file's language code is not match to target_language, set it to target_language
        if !matches!(&target_content.language(), Some(lang) if lang == &target_language) {
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
        target_ts.language = Some("zh_TW".to_string());
        target_ts.clear_finished_messages();
        assert!(translate_ts_content(&source_ts, &mut target_ts).is_ok());
        assert_eq!(target_ts.language, Some("zh_TW".to_string()));
        assert_eq!(target_ts.contexts.len(), 1);
        assert_eq!(target_ts.contexts[0].messages.len(), 4);
        assert_eq!(target_ts.contexts[0].messages[0].translation.value, Some(String::from("海內存知己")));
        assert_eq!(target_ts.contexts[0].messages[1].translation.value, Some(String::from("軟體開發工程師在使用滑鼠操作螢幕上的游標")));
        assert_eq!(target_ts.contexts[0].messages[2].translation.value, Some(String::from("电视频段"))); // marked as obsolete, should not be translated.
        assert_eq!(target_ts.contexts[0].messages[3].translation.value, None); // source is also untranslated
    }
}

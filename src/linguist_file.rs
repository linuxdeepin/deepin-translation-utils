// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

// Linguist .ts XML file spec: https://doc.qt.io/qt-6/linguist-ts-file-format.html

use std::fs;
use std::ops;
use std::path::PathBuf;
use std::u64;

use thiserror::Error as TeError;
use quick_xml::DeError;
use serde::Deserialize;
use serde::Serialize;
use quick_xml::se::SeError;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesText, Event};

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct TsMessageStats {
    pub finished: u64,
    pub unfinished: u64,
    pub vanished: u64,
    pub obsolete: u64,
}

impl TsMessageStats {
    pub fn completeness_percentage(&self) -> f64 {
        let total = self.finished + self.unfinished;
        if total == 0 {
            return 0.0;
        }
        (self.finished as f64 / total as f64) * 100.0
    }
}

impl ops::AddAssign<&Self> for TsMessageStats {
    fn add_assign(&mut self, rhs: &Self) {
        self.finished += rhs.finished;
        self.unfinished += rhs.unfinished;
        self.vanished += rhs.vanished;
        self.obsolete += rhs.obsolete;
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename = "TS")]
pub struct Ts {
    #[serde(rename = "@language")]
    pub language: Option<String>,
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "context")]
    pub contexts: Vec<Context>,
}

impl Ts {
    pub fn clear_finished_messages(&mut self) {
        for context in &mut self.contexts {
            for message in &mut context.messages {
                if message.translation.type_attr.is_some() {
                    continue;
                }
                message.translation.value = None;
                message.translation.type_attr = Some(TranslationType::Unfinished);
            }
        }
    }

    pub fn set_language(&mut self, language: &String) {
        self.language = Some(language.clone());
    }

    pub fn get_message_stats(&self) -> TsMessageStats {
        let mut finished = 0;
        let mut unfinished = 0;
        let mut vanished = 0;
        let mut obsolete = 0;
        for context in &self.contexts {
            for message in &context.messages {
                match message.translation.type_attr {
                    Some(TranslationType::Unfinished) => {
                        unfinished += 1;
                    }
                    Some(TranslationType::Vanished) => {
                        vanished += 1;
                    }
                    Some(TranslationType::Obsolete) => {
                        obsolete += 1;
                    }
                    None => {
                        finished += 1;
                    }
                }
            }
        }
        return TsMessageStats {
            finished,
            unfinished,
            vanished,
            obsolete,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Context {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "message")]
    pub messages: Vec<Message>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Message {
    #[serde(rename = "location", default)]
    pub location: Vec<Location>,
    #[serde(rename = "source")]
    pub source: String,
    #[serde(rename = "translation")]
    pub translation: Translation,
    #[serde(rename = "comment", skip_serializing_if = "Option::is_none", default)]
    pub comment: Option<String>,
    #[serde(rename = "@numerus", skip_serializing_if = "Option::is_none", default)]
    pub numerus: Option<String>,
}

impl Message {
    pub fn fill_translation(&mut self, translation: &String) {
        self.translation.value = Some(translation.clone());
        self.translation.type_attr = None;
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum TranslationType {
    Unfinished,
    Vanished,
    Obsolete,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Translation {
    #[serde(rename = "@type", skip_serializing_if = "Option::is_none", default)]
    pub type_attr: Option<TranslationType>,
    #[serde(rename = "$value")]
    pub value: Option<String>,
    #[serde(rename = "numerusform", default)]
    pub numerus_forms: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Location {
    #[serde(rename = "@filename", skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(rename = "@line")]
    pub line: String,
}

#[derive(TeError, Debug)]
pub enum TsLoadError {
    #[error("File not found")]
    FileNotFound,
    #[error("Can not read file")]
    ReadFile(#[from] std::io::Error),
    #[error("Fail to deserialize file: {0}")]
    Serde(#[from] DeError),
}

pub fn correct_language_code(language_code: &String) -> String {
    let mut result = language_code.clone();
    result = result.replace("_", "-");
    return result;
}

pub fn load_ts_file_or_default(linguist_ts_file: &PathBuf, fallback: &Ts, fallback_language_code: &String) -> Result<Ts, TsLoadError> {
    if !linguist_ts_file.exists() {
        let mut clone = fallback.clone();
        clone.language = Some(fallback_language_code.clone());
        clone.clear_finished_messages();
        return Ok(clone);
    } else {
        return load_ts_file(linguist_ts_file);
    }
}

pub fn load_ts_file(linguist_ts_file: &PathBuf) -> Result<Ts, TsLoadError> {
    if !linguist_ts_file.is_file() {
        return Err(TsLoadError::FileNotFound);
    }
    let source_content = fs::read_to_string(&linguist_ts_file)?;
    Ok(quick_xml::de::from_str::<Ts>(source_content.as_str())?)
}

pub trait WriterExt {
    fn write_linguist_ts_file(
        &mut self,
        content: &Ts,
    ) -> Result<(), SeError>;
}

impl<W: std::io::Write> WriterExt for Writer<W> {
    fn write_linguist_ts_file(
        &mut self,
        content: &Ts,
    ) -> Result<(), SeError> {
        self.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
        self.write_event(Event::DocType(BytesText::new("TS")))?;
        self.write_serializable("TS", content)
    }
}

#[derive(TeError, Debug)]
pub enum TsSaveError {
    #[error("Can not create file")]
    CreateFile(#[from] std::io::Error),
    #[error("Fail to serialize file: {0}")]
    Serde(#[from] SeError),
}

pub fn save_ts_file(linguist_ts_file: &PathBuf, content: &Ts) -> Result<(), TsSaveError> {
    let target_file = fs::File::create(linguist_ts_file)?;
    let mut writer = Writer::new_with_indent(&target_file, b' ', 4);
    writer.write_linguist_ts_file(content)?;
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const TEST_ZH_CN_TS_CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<?xml version="1.0" ?><!DOCTYPE TS><TS language="zh_CN" version="2.1">
<context>
    <name>ts::SampleContext</name>
    <message>
        <location filename="../../widget/mainwindow.ui" line="+17"/>
        <source>A friend in need is a friend indeed</source>
        <translation>海内存知己</translation>
    </message>
    <message>
        <location line="+26"/>
        <location filename="../../widget/mainwindow.cpp" line="+65"/>
        <source>Software engineer using mouse to manipulate the cursor on the screen</source>
        <translation>软件开发工程师在使用鼠标操作屏幕上的光标</translation>
    </message>
    <message>
        <source>TV band</source>
        <translation type="obsolete">电视频段</translation>
    </message>
    <message>
        <source>England</source>
        <translation type="unfinished"/>
    </message>
    <message numerus="yes">
        <source>%n photos</source>
        <translation><numerusform>共%n张照片</numerusform></translation>
    </message>
</context>
</TS>"#;

    #[test]
    fn tst_parse_ts_content() {
        let ts: Ts = quick_xml::de::from_str(TEST_ZH_CN_TS_CONTENT).unwrap();
        assert_eq!(ts.language, Some("zh_CN".to_string()));
        assert_eq!(ts.version, "2.1");
        assert_eq!(ts.contexts.len(), 1);
        assert_eq!(ts.contexts[0].name, "ts::SampleContext");
        assert_eq!(ts.contexts[0].messages.len(), 5);
        assert!(matches!(ts.contexts[0].messages[1].translation.type_attr, None));
        assert!(matches!(ts.contexts[0].messages[2].translation.type_attr, Some(TranslationType::Obsolete)));
        assert!(matches!(ts.contexts[0].messages[3].translation.type_attr, Some(TranslationType::Unfinished)));
        assert_eq!(ts.get_message_stats(), TsMessageStats {
            finished: 3,
            unfinished: 1,
            vanished: 0,
            obsolete: 1,
        });
        assert_eq!(ts.get_message_stats().completeness_percentage(), 3.0 / 4.0 * 100.0);
    }
}

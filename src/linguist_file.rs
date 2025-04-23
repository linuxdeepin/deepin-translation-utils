// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

// Linguist .ts XML file spec: https://doc.qt.io/qt-6/linguist-ts-file-format.html

use std::fs;
use std::path::PathBuf;

use thiserror::Error as TeError;
use quick_xml::DeError;
use serde::Deserialize;
use serde::Serialize;
use quick_xml::se::SeError;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesText, Event};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename = "TS")]
pub struct Ts {
    #[serde(rename = "@language")]
    pub language: String,
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "context")]
    pub contexts: Vec<Context>,
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
    #[serde(rename = "source")]
    pub source: String,
    #[serde(rename = "translation")]
    pub translation: Translation,
    #[serde(rename = "location", skip_serializing_if = "Option::is_none", default)]
    pub location: Option<Location>,
    #[serde(rename = "comment", skip_serializing_if = "Option::is_none", default)]
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Translation {
    #[serde(rename = "@type", skip_serializing_if = "Option::is_none", default)]
    pub type_attr: Option<String>,
    #[serde(rename = "$value")]
    pub value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Location {
    #[serde(rename = "@filename")]
    pub filename: String,
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

fn mark_all_as_unfinished(ts: &mut Ts) {
    for context in &mut ts.contexts {
        for message in &mut context.messages {
            message.translation.value = None;
            message.translation.type_attr = Some("unfinished".to_string());
        }
    }
}

pub fn correct_language_code(language_code: &String) -> String {
    let mut result = language_code.clone();
    result = result.replace("_", "-");
    return result;
}

pub fn load_ts_file_or_default(linguist_ts_file: &PathBuf, fallback: &Ts, fallback_language_code: &String) -> Result<Ts, TsLoadError> {
    if !linguist_ts_file.exists() {
        let mut clone = fallback.clone();
        clone.language = fallback_language_code.clone();
        mark_all_as_unfinished(&mut clone);
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

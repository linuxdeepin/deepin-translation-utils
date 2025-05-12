// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use std::path::Path;
use polib::message::{MessageMutView, MessageView};
use polib::po_file::{self, POParseError};
use thiserror::Error as TeError;
use super::common::MessageStats;

// ===== PO Basic =====

#[derive(Debug, Clone)]
pub struct Po {
    pub inner: polib::catalog::Catalog,
}

impl Po {
    pub fn clear_finished_messages(&mut self) {
        let catalog = &mut self.inner;
        for mut message in catalog.messages_mut() {
            if message.is_translated() && !message.is_plural() {
                // This can not be failed because we have checked whether it is plural.
                // Unwrap directly.
                message.set_msgstr(String::new()).unwrap();
            }
        }
    }
}

impl Po {
    pub fn get_language(&self) -> String {
        self.inner.metadata.language.clone()
    }

    pub fn set_language(&mut self, language: &str) {
        self.inner.metadata.language = language.to_string();
    }

    pub fn get_message_stats(&self) -> MessageStats {
        let mut stats = MessageStats::new();
        for message in self.inner.messages() {
            if message.is_translated() {
                stats.finished += 1;
            } else if message.is_fuzzy() {
                stats.fuzzy += 1;
            } else {
                stats.unfinished += 1;
            }
        }
        return stats;
    }
}

// ===== PO Load & Save =====

#[derive(TeError, Debug)]
pub enum PoLoadError {
    #[error("Fail to parse PO file: {0}")]
    ParsePo(#[from] POParseError),
}

#[derive(TeError, Debug)]
pub enum PoSaveError {
    #[error("Fail to save PO file: {0}")]
    WritePo(#[from] std::io::Error),
}

impl Po {
    pub fn load_from_file(po_file: &Path) -> Result<Po, PoLoadError> {
        Ok(Po {
            inner: po_file::parse(po_file)?,
        })
    }

    #[cfg(test)]
    pub fn load_from_str(content: &str) -> Result<Po, PoLoadError> {
        let reader = std::io::Cursor::new(content.as_bytes());
        Ok(Po {
            inner: po_file::parse_from_reader(reader)?
        })
    }

    pub fn load_from_file_or_default(po_file: &Path, fallback: &Po, fallback_language_code: &str) -> Result<Po, PoLoadError> {
        if !po_file.exists() {
            let mut po = fallback.clone();
            po.set_language(fallback_language_code);
            po.clear_finished_messages();
            return Ok(po);
        } else {
            return Self::load_from_file(po_file);
        }
    }

    pub fn save_into_file(&self, po_file: &Path) -> Result<(), PoSaveError> {
        po_file::write_to_file(&self.inner, po_file)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::super::common::MessageStats;
    use super::*;

    pub const TEST_ZH_CN_PO_CONTENT: &str = r#"msgid ""
msgstr ""
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"
"Plural-Forms: nplurals=1; plural=0;\n"
"Language: zh_CN\n"
"X-Source-Language: C\n"
"X-Qt-Contexts: true\n"

#: ../../widget/mainwindow.ui:17
msgctxt "ts::SampleContext|"
msgid "A friend in need is a friend indeed"
msgstr "海内存知己"

#: ../../widget/mainwindow.ui:43 ../../widget/mainwindow.cpp:65
msgctxt "ts::SampleContext|"
msgid "Software engineer using mouse to manipulate the cursor on the screen"
msgstr "软件开发工程师在使用鼠标操作屏幕上的光标"

#, fuzzy
#~ msgctxt "ts::SampleContext|"
#~ msgid "TV band"
#~ msgstr "电视频段"

msgctxt "ts::SampleContext|"
msgid "England"
msgstr ""
"#;

    #[test]
    fn tst_parse_po_content() {
        let po = Po::load_from_str(TEST_ZH_CN_PO_CONTENT).unwrap();
        assert_eq!(po.get_language(), "zh_CN");
        assert_eq!(po.get_message_stats(), MessageStats {
            finished: 2,
            unfinished: 1,
            vanished: 0,
            obsolete: 0,
            fuzzy: 1,
        });
        assert_eq!(po.get_message_stats().completeness_percentage(None), 2.0 / 4.0 * 100.0);
    }
}
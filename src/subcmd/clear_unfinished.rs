// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

//! The `clear-unfinished` subcommand.
//!
//! Scan the project (following `transifex.yaml` / `.tx/config`) for translation
//! entries which are marked as unfinished but actually contain translation
//! content, remove those marks and save the files back.
//!
//! - Qt Linguist `.ts`: entries with `type="unfinished"` and a non-empty
//!   translation (or all-non-empty `numerusform`s for plural entries) get their
//!   `type` attribute removed. Other types (vanished, obsolete) are untouched.
//! - GNU Gettext `.po`: entries carrying the `fuzzy` flag with non-empty
//!   `msgstr` (or all-non-empty `msgstr[N]` for plural entries) get the flag
//!   removed.
//!
//! Entries without any translation content are always left untouched.

use polib::message::{MessageMutView, MessageView};
use std::path::{Path, PathBuf};
use thiserror::Error as TeError;

use crate::i18n_file::{
    gettext::Po,
    linguist::{Translation, TranslationType, Ts},
};
use crate::subcmd::resources::{collect_resource_paths, lang_kind_from_path, ResourceError};

// ===== Error =====

#[derive(TeError, Debug)]
pub enum CmdError {
    #[error("Fail to collect translation resources because: {0}")]
    CollectResources(#[from] ResourceError),
    #[error("Fail to load Qt Linguist TS file {0:?} because: {1}")]
    LoadTsFile(PathBuf, #[source] crate::i18n_file::linguist::TsLoadError),
    #[error("Fail to load Gettext PO file {0:?} because: {1}")]
    LoadPoFile(PathBuf, #[source] crate::i18n_file::gettext::PoLoadError),
    #[error("Fail to save Qt Linguist TS file {0:?} because: {1}")]
    SaveTsFile(PathBuf, #[source] crate::i18n_file::linguist::TsSaveError),
    #[error("Fail to save Gettext PO file {0:?} because: {1}")]
    SavePoFile(PathBuf, #[source] crate::i18n_file::gettext::PoSaveError),
}

// ===== Main =====

pub fn subcmd_clear_unfinished(
    project_root: &PathBuf,
    accept_languages: &[String],
) -> Result<(), CmdError> {
    let mut total_entries = 0usize;
    let mut total_files = 0usize;

    for path in collect_resource_paths(project_root, accept_languages)? {
        let kind = lang_kind_from_path(&path)?;
        let cleared = match kind.as_str() {
            "ts" => clear_ts_file(&path)?,
            "po" => clear_po_file(&path)?,
            _ => 0,
        };
        if cleared > 0 {
            total_files += 1;
            total_entries += cleared;
            println!("{}: cleared unfinished mark of {} entries", path.display(), cleared);
        }
    }

    println!(
        "Cleared unfinished marks of {} entries in {} files.",
        total_entries, total_files
    );
    Ok(())
}

// ===== TS =====

fn clear_ts_file(path: &Path) -> Result<usize, CmdError> {
    let mut ts = Ts::load_from_file(path).map_err(|e| CmdError::LoadTsFile(path.to_path_buf(), e))?;
    let mut cleared = 0usize;
    for context in ts.contexts.iter_mut() {
        for message in context.messages.iter_mut() {
            if !matches!(message.translation.type_attr, Some(TranslationType::Unfinished)) {
                continue;
            }
            if !ts_translation_has_content(&message.translation) {
                continue;
            }
            message.translation.type_attr = None;
            cleared += 1;
        }
    }
    if cleared > 0 {
        ts.save_into_file(path)
            .map_err(|e| CmdError::SaveTsFile(path.to_path_buf(), e))?;
    }
    Ok(cleared)
}

/// Whether the unfinished translation actually carries content.
///
/// A singular entry needs a non-empty value; a plural entry needs every
/// numerusform to be non-empty (partially filled plurals stay unfinished).
fn ts_translation_has_content(translation: &Translation) -> bool {
    if translation.value.as_deref().is_some_and(|v| !v.is_empty()) {
        return true;
    }
    !translation.numerus_forms.is_empty()
        && translation.numerus_forms.iter().all(|f| !f.is_empty())
}

// ===== PO =====

fn clear_po_file(path: &Path) -> Result<usize, CmdError> {
    let mut po = Po::load_from_file(path).map_err(|e| CmdError::LoadPoFile(path.to_path_buf(), e))?;
    let mut cleared = 0usize;
    for mut message in po.inner.messages_mut() {
        if !message.is_fuzzy() {
            continue;
        }
        if !po_message_has_content(&message) {
            continue;
        }
        message.flags_mut().remove_flag("fuzzy");
        cleared += 1;
    }
    if cleared > 0 {
        po.save_into_file(path)
            .map_err(|e| CmdError::SavePoFile(path.to_path_buf(), e))?;
    }
    Ok(cleared)
}

/// Whether the fuzzy message actually carries content.
///
/// A singular message needs a non-empty msgstr; a plural message needs every
/// msgstr[N] to be non-empty (partially filled plurals stay fuzzy).
fn po_message_has_content<M: MessageView>(message: &M) -> bool {
    if message.is_plural() {
        message
            .msgstr_plural()
            .map(|forms| !forms.is_empty() && forms.iter().all(|f| !f.is_empty()))
            .unwrap_or(false)
    } else {
        message.msgstr().map(|s| !s.is_empty()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp_file(dir_suffix: &str, file_name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dtt-clear-{}-{}-{}",
            dir_suffix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(file_name);
        std::fs::write(&path, content).unwrap();
        path
    }

    const TS_WITH_CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE TS><TS language="zh_CN" version="2.1">
<context>
    <name>C</name>
    <message>
        <source>Hello</source>
        <translation type="unfinished">你好</translation>
    </message>
    <message numerus="yes">
        <source>%n photos</source>
        <translation type="unfinished"><numerusform>共%n张照片</numerusform></translation>
    </message>
</context>
</TS>"#;

    #[test]
    fn tst_ts_clears_unfinished_with_content() {
        let path = write_temp_file("ts1", "a.ts", TS_WITH_CONTENT);

        let cleared = clear_ts_file(&path).unwrap();
        assert_eq!(cleared, 2);

        let ts = Ts::load_from_file(&path).unwrap();
        for msg in &ts.contexts[0].messages {
            assert!(msg.translation.type_attr.is_none());
        }
        assert_eq!(ts.contexts[0].messages[0].translation.value.as_deref(), Some("你好"));
        assert_eq!(ts.contexts[0].messages[1].translation.numerus_forms, vec!["共%n张照片"]);
    }

    #[test]
    fn tst_ts_keeps_unfinished_without_content() {
        let path = write_temp_file("ts2", "b.ts", r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE TS><TS language="zh_CN" version="2.1">
<context>
    <name>C</name>
    <message>
        <source>Empty</source>
        <translation type="unfinished"/>
    </message>
    <message>
        <source>Vanished</source>
        <translation type="vanished">消失</translation>
    </message>
    <message>
        <source>Partial plural</source>
        <translation type="unfinished"><numerusform></numerusform><numerusform></numerusform></translation>
    </message>
</context>
</TS>"#);

        let cleared = clear_ts_file(&path).unwrap();
        assert_eq!(cleared, 0);

        let ts = Ts::load_from_file(&path).unwrap();
        let messages = &ts.contexts[0].messages;
        assert!(matches!(messages[0].translation.type_attr, Some(TranslationType::Unfinished)));
        assert!(matches!(messages[1].translation.type_attr, Some(TranslationType::Vanished)));
        assert!(matches!(messages[2].translation.type_attr, Some(TranslationType::Unfinished)));
    }

    const PO_WITH_CONTENT: &str = r#"msgid ""
msgstr ""
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"

#, fuzzy
msgid "Hello"
msgstr "你好"

#, fuzzy
msgid "Empty"
msgstr ""

msgid "Translated"
msgstr "已翻译"
"#;

    #[test]
    fn tst_po_clears_fuzzy_with_content() {
        let path = write_temp_file("po1", "c.po", PO_WITH_CONTENT);

        let cleared = clear_po_file(&path).unwrap();
        assert_eq!(cleared, 1);

        let po = Po::load_from_file(&path).unwrap();
        let stats = po.get_message_stats();
        assert_eq!(stats.fuzzy, 1);
        assert_eq!(stats.finished, 2);
        assert_eq!(stats.unfinished, 0);
    }

    #[test]
    fn tst_po_plural_fuzzy_requires_all_forms() {
        let plural = |forms: &[&str]| {
            format!(
                "msgid \"\"\nmsgstr \"\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\n#, fuzzy\nmsgid \"%n photo\"\nmsgid_plural \"%n photos\"\nmsgstr[0] \"{}\"\nmsgstr[1] \"{}\"\n",
                forms[0], forms[1]
            )
        };

        // All forms filled -> flag removed.
        let path = write_temp_file("po2", "d.po", &plural(&["一", "多"]));
        assert_eq!(clear_po_file(&path).unwrap(), 1);
        assert_eq!(Po::load_from_file(&path).unwrap().get_message_stats().fuzzy, 0);

        // Any form empty -> stays fuzzy.
        let path = write_temp_file("po3", "e.po", &plural(&["一", ""]));
        assert_eq!(clear_po_file(&path).unwrap(), 0);
        assert_eq!(Po::load_from_file(&path).unwrap().get_message_stats().fuzzy, 1);
    }
}

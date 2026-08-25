// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

//! The `fill-translations` subcommand.
//!
//! This is a two-phase I/O pipeline designed to be driven by an external
//! translation provider (e.g. an AI tool):
//!
//! 1. `export`: scan every resource of the project (following `transifex.yaml` /
//!    `.tx/config`) for a given target language and emit a JSON document listing
//!    every *unfinished* message (source text, context, placeholder info and
//!    plural-form count). Nothing is modified on disk.
//!
//! 2. `apply`: consume the same JSON document (produced by `export`) with the
//!    `translation` field(s) filled in, validate each translation preserves the
//!    source placeholders and the expected number of plural forms, then fill
//!    them back into the matching `.ts` / `.po` files, marking them finished.

use polib::message::{MessageMutView, MessageView};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error as TeError;

use crate::i18n_file::{
    self,
    placeholder,
    gettext::Po,
    linguist::{TranslationType, Ts},
};
use crate::transifex::project_file::{try_load_transifex_project_file, TxProjectFileLoadError};

// ===== Error =====

#[derive(TeError, Debug)]
pub enum CmdError {
    #[error("Fail to load Transifex project file because: {0}")]
    LoadTxProjectFile(#[from] TxProjectFileLoadError),
    #[error("Fail to match resources because: {0}")]
    MatchResources(#[source] std::io::Error),
    #[error("Can not guess translation file kind from path {0:?} because: {1}")]
    GuessI18nFileType(PathBuf, #[source] i18n_file::common::UnknownI18nFileExtError),
    #[error("Fail to load Qt Linguist TS file {0:?} because: {1}")]
    LoadTsFile(PathBuf, #[source] i18n_file::linguist::TsLoadError),
    #[error("Fail to load Gettext PO file {0:?} because: {1}")]
    LoadPoFile(PathBuf, #[source] i18n_file::gettext::PoLoadError),
    #[error("Fail to save Qt Linguist TS file {0:?} because: {1}")]
    SaveTsFile(PathBuf, #[source] i18n_file::linguist::TsSaveError),
    #[error("Fail to save Gettext PO file {0:?} because: {1}")]
    SavePoFile(PathBuf, #[source] i18n_file::gettext::PoSaveError),
    #[error("Target language code is empty")]
    EmptyTargetLanguage,
    #[error("The provided file {0:?} does not exist")]
    FileNotFound(PathBuf),
    #[error("Fail to read file {0:?} because: {1}")]
    ReadFile(PathBuf, #[source] std::io::Error),
    #[error("Fail to parse JSON payload: {0}")]
    ParseJson(#[from] serde_json::Error),
    #[error("Fail to serialize JSON: {0}")]
    SerializeJson(serde_json::Error),
    #[error("The JSON document does not declare a project root")]
    MissingProjectRoot,
    #[error("The JSON document declares project root {0:?} but provided project root is {1:?}")]
    ProjectRootMismatch(PathBuf, PathBuf),
    #[error("Translation for source {0:?} in context {1:?} does not preserve the source placeholders")]
    PlaceholderMismatch(String, String),
    #[error("Wrong number of plural forms for source {0:?} in context {1:?}: expected {2}, got {3}")]
    PluralFormCountMismatch(String, String, usize, usize),
    #[error("Entry for source {0:?} in context {1:?} is plural but a singular translation was provided")]
    ExpectedPlural(String, String),
    #[error("Entry for source {0:?} in context {1:?} is singular but a plural translation was provided")]
    ExpectedSingular(String, String),
}

// ===== JSON document model =====

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FillDocument {
    /// Absolute project root path this document was exported for.
    pub project_root: PathBuf,
    /// Target language code these translations belong to.
    pub target_language: String,
    /// Source language code.
    pub source_language: String,
    /// Per-resource entries.
    #[serde(default)]
    pub resources: Vec<FillResource>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FillResource {
    /// Absolute path to the translation resource file (`.ts` or `.po`).
    pub path: PathBuf,
    /// `ts` or `po`.
    pub kind: String,
    /// Unfinished entries.
    #[serde(default)]
    pub entries: Vec<FillEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillEntry {
    /// Message scope. For `.ts` this is the `<context>` name; for `.po` it is
    /// the `msgctxt` (empty string when none).
    pub context: String,
    /// The source string (`<source>` / `msgid`).
    pub source: String,
    /// Placeholders found in the source (informational; set on export).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placeholders: Vec<String>,
    /// Whether the source is a plural message.
    #[serde(default)]
    pub plural: bool,
    /// Number of plural forms expected (set on export; validated on apply).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plural_count: Option<usize>,
    /// The translation(s). Singular -> a single string; plural -> one string
    /// per plural form. Filled in by the caller on `apply`.
    pub translation: Option<FillTranslation>,
}

impl FillEntry {
    /// Whether this entry currently lacks a (complete) filled translation.
    fn is_unfilled(&self) -> bool {
        match &self.translation {
            Some(FillTranslation::Singular(s)) => s.is_empty(),
            Some(FillTranslation::Plural(forms)) => forms.is_empty() || forms.iter().any(|f| f.is_empty()),
            None => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FillTranslation {
    Singular(String),
    Plural(Vec<String>),
}

// ===== Resources discovery =====

fn lang_kind_from_path(path: &Path) -> Result<String, CmdError> {
    use i18n_file::common::I18nFileKind;
    match I18nFileKind::from_ext_hint(path) {
        Ok(I18nFileKind::Linguist) => Ok("ts".to_string()),
        Ok(I18nFileKind::Gettext) => Ok("po".to_string()),
        Err(e) => Err(CmdError::GuessI18nFileType(path.to_path_buf(), e)),
    }
}

/// Collect the target-language resource file paths for every supported filter.
fn collect_resource_paths(project_root: &PathBuf, target_language: &str) -> Result<Vec<PathBuf>, CmdError> {
    let (_, tx_yaml) = try_load_transifex_project_file(project_root)?;
    let mut rv = Vec::new();
    for filter in &tx_yaml.filters {
        if (filter.format != "QT" && filter.format != "PO") || filter.type_attr != "file" {
            continue;
        }
        let matched = filter
            .match_target_files(project_root)
            .map_err(CmdError::MatchResources)?;
        rv.extend(
            matched
                .into_iter()
                .filter_map(|(lang, path)| (lang == target_language).then_some(path)),
        );
    }
    Ok(rv)
}

// ===== Export =====

/// Take at most `limit` entries from `entries`, returning the taken entries and
/// the number consumed. Used to support `--limit` for batching.
fn truncate_entries(entries: Vec<FillEntry>, limit: usize) -> (Vec<FillEntry>, usize) {
    let take = entries.len().min(limit);
    (entries.into_iter().take(take).collect(), take)
}

pub fn subcmd_fill_export(
    project_root: &PathBuf,
    target_language: &str,
    source_language: &str,
    limit: Option<usize>,
) -> Result<(), CmdError> {
    if target_language.is_empty() {
        return Err(CmdError::EmptyTargetLanguage);
    }

    let mut doc = FillDocument {
        project_root: project_root.clone(),
        target_language: target_language.to_string(),
        source_language: source_language.to_string(),
        resources: Vec::new(),
    };

    let mut remaining = limit.unwrap_or(usize::MAX);

    for path in collect_resource_paths(project_root, target_language)? {
        if remaining == 0 {
            break;
        }
        let kind = lang_kind_from_path(&path)?;
        let entries = match kind.as_str() {
            "ts" => export_ts_entries(&path)?,
            "po" => export_po_entries(&path)?,
            _ => Vec::new(),
        };
        if entries.is_empty() {
            continue;
        }
        let (entries, consumed) = truncate_entries(entries, remaining);
        remaining -= consumed;
        doc.resources.push(FillResource {
            path,
            kind,
            entries,
        });
    }

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

fn export_ts_entries(path: &Path) -> Result<Vec<FillEntry>, CmdError> {
    let ts = Ts::load_from_file(path).map_err(|e| CmdError::LoadTsFile(path.to_path_buf(), e))?;
    Ok(ts
        .contexts
        .iter()
        .flat_map(|ctx| {
            ctx.messages.iter().filter_map(|msg| {
                if !matches!(msg.translation.type_attr, Some(TranslationType::Unfinished)) {
                    return None;
                }
                let plural = msg.numerus.as_deref() == Some("yes");
                Some(FillEntry {
                    context: ctx.name.clone(),
                    source: msg.source.clone(),
                    placeholders: placeholder::extract_placeholders(&msg.source),
                    plural,
                    plural_count: plural.then_some(msg.translation.numerus_forms.len()),
                    translation: None,
                })
            })
        })
        .collect())
}

fn export_po_entries(path: &Path) -> Result<Vec<FillEntry>, CmdError> {
    let po = Po::load_from_file(path).map_err(|e| CmdError::LoadPoFile(path.to_path_buf(), e))?;
    Ok(po
        .inner
        .messages()
        .filter_map(|msg| {
            if msg.is_translated() {
                return None;
            }
            let plural = msg.is_plural();
            Some(FillEntry {
                context: msg.msgctxt().unwrap_or("").to_string(),
                source: msg.msgid().to_string(),
                placeholders: placeholder::extract_placeholders(msg.msgid()),
                plural,
                plural_count: plural.then(|| msg.msgstr_plural().map(|f| f.len()).unwrap_or(0)),
                translation: None,
            })
        })
        .collect())
}

// ===== Apply =====

pub fn subcmd_fill_apply(project_root: &Path, json_file: &Path) -> Result<(), CmdError> {
    if !json_file.is_file() {
        return Err(CmdError::FileNotFound(json_file.to_path_buf()));
    }
    let content = std::fs::read_to_string(json_file)
        .map_err(|e| CmdError::ReadFile(json_file.to_path_buf(), e))?;
    let doc: FillDocument = serde_json::from_str(&content)?;

    if doc.project_root.as_os_str().is_empty() {
        return Err(CmdError::MissingProjectRoot);
    }

    // Verify the payload's project root matches the one given on the command line.
    let doc_root = doc.project_root.canonicalize().unwrap_or(doc.project_root.clone());
    let given_root = project_root.canonicalize().unwrap_or_else(|_| project_root.to_path_buf());
    if doc_root != given_root {
        return Err(CmdError::ProjectRootMismatch(doc_root, given_root));
    }

    for resource in &doc.resources {
        apply_resource(resource)?;
    }

    Ok(())
}

fn apply_resource(resource: &FillResource) -> Result<(), CmdError> {
    let to_fill: Vec<&FillEntry> = resource
        .entries
        .iter()
        .filter(|e| !e.is_unfilled())
        .collect();
    if to_fill.is_empty() {
        return Ok(());
    }
    match resource.kind.as_str() {
        "ts" => apply_ts_resource(resource, &to_fill),
        "po" => apply_po_resource(resource, &to_fill),
        _ => Ok(()),
    }
}

fn apply_ts_resource(resource: &FillResource, to_fill: &[&FillEntry]) -> Result<(), CmdError> {
    let mut ts = Ts::load_from_file(&resource.path)
        .map_err(|e| CmdError::LoadTsFile(resource.path.clone(), e))?;
    let mut filled = 0usize;
    let mut missing: Vec<&FillEntry> = Vec::new();

    'outer: for entry in to_fill.iter().copied() {
        for ctx in ts.contexts.iter_mut() {
            if ctx.name != entry.context {
                continue;
            }
            for msg in ctx.messages.iter_mut() {
                if msg.source != entry.source {
                    continue;
                }
                // Only finish messages that are still unfinished.
                if !matches!(msg.translation.type_attr, Some(TranslationType::Unfinished)) {
                    continue;
                }
                let translation = entry.translation.as_ref().unwrap();
                match (entry.plural, translation) {
                    (true, FillTranslation::Plural(forms)) => {
                        let expected = entry.plural_count.unwrap_or(forms.len());
                        if forms.len() != expected {
                            return Err(CmdError::PluralFormCountMismatch(
                                entry.source.clone(), entry.context.clone(), expected, forms.len(),
                            ));
                        }
                        for f in forms {
                            if !placeholder::placeholders_match(&entry.source, f) {
                                return Err(CmdError::PlaceholderMismatch(
                                    entry.source.clone(), entry.context.clone(),
                                ));
                            }
                        }
                        msg.translation.numerus_forms = forms.clone();
                        msg.translation.type_attr = None;
                    }
                    (false, FillTranslation::Singular(value)) => {
                        if !placeholder::placeholders_match(&entry.source, value) {
                            return Err(CmdError::PlaceholderMismatch(
                                entry.source.clone(), entry.context.clone(),
                            ));
                        }
                        msg.fill_translation(value);
                    }
                    (true, _) => {
                        return Err(CmdError::ExpectedPlural(entry.source.clone(), entry.context.clone()));
                    }
                    (false, _) => {
                        return Err(CmdError::ExpectedSingular(entry.source.clone(), entry.context.clone()));
                    }
                }
                filled += 1;
                continue 'outer;
            }
        }
        missing.push(entry);
    }

    if filled > 0 {
        ts.save_into_file(&resource.path)
            .map_err(|e| CmdError::SaveTsFile(resource.path.clone(), e))?;
    }
    for m in missing {
        eprintln!("Warning: no unfinished match for {}/{}", m.context, m.source);
    }
    Ok(())
}

fn apply_po_resource(resource: &FillResource, to_fill: &[&FillEntry]) -> Result<(), CmdError> {
    let mut po = Po::load_from_file(&resource.path)
        .map_err(|e| CmdError::LoadPoFile(resource.path.clone(), e))?;
    let mut filled = 0usize;

    for entry in to_fill.iter().copied() {
        let mut found = false;
        for mut msg in po.inner.messages_mut() {
            if msg.msgid() != entry.source {
                continue;
            }
            let ctx_matches = match (msg.msgctxt(), entry.context.as_str()) {
                (Some(c), ec) => c == ec,
                (None, "") => true,
                _ => false,
            };
            if !ctx_matches {
                continue;
            }
            if msg.is_translated() {
                continue;
            }

            let translation = entry.translation.as_ref().unwrap();
            match (entry.plural, translation) {
                (true, FillTranslation::Plural(forms)) => {
                    if !msg.is_plural() {
                        return Err(CmdError::ExpectedPlural(entry.source.clone(), entry.context.clone()));
                    }
                    let expected = entry.plural_count.unwrap_or(forms.len());
                    if forms.len() != expected {
                        return Err(CmdError::PluralFormCountMismatch(
                            entry.source.clone(), entry.context.clone(), expected, forms.len(),
                        ));
                    }
                    for f in forms {
                        if !placeholder::placeholders_match(entry.source.trim_end(), f) {
                            return Err(CmdError::PlaceholderMismatch(
                                entry.source.clone(), entry.context.clone(),
                            ));
                        }
                    }
                    let msgstr_plural = msg.msgstr_plural_mut().map_err(|_| {
                        CmdError::ExpectedPlural(entry.source.clone(), entry.context.clone())
                    })?;
                    msgstr_plural.clear();
                    msgstr_plural.extend(forms.iter().cloned());
                    msg.flags_mut().remove_flag("fuzzy");
                }
                (false, FillTranslation::Singular(value)) => {
                    if msg.is_plural() {
                        return Err(CmdError::ExpectedSingular(entry.source.clone(), entry.context.clone()));
                    }
                    if !placeholder::placeholders_match(&entry.source, value) {
                        return Err(CmdError::PlaceholderMismatch(
                            entry.source.clone(), entry.context.clone(),
                        ));
                    }
                    msg.set_msgstr(value.clone()).map_err(|_| {
                        CmdError::ExpectedSingular(entry.source.clone(), entry.context.clone())
                    })?;
                    msg.flags_mut().remove_flag("fuzzy");
                }
                (true, _) => {
                    return Err(CmdError::ExpectedPlural(entry.source.clone(), entry.context.clone()));
                }
                (false, _) => {
                    return Err(CmdError::ExpectedSingular(entry.source.clone(), entry.context.clone()));
                }
            }
            found = true;
            filled += 1;
            break;
        }
        if !found {
            eprintln!(
                "Warning: no unfinished match for {}/{} in {}",
                entry.context,
                entry.source,
                resource.path.display()
            );
        }
    }

    if filled > 0 {
        po.save_into_file(&resource.path)
            .map_err(|e| CmdError::SavePoFile(resource.path.clone(), e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n_file::linguist::tests::TEST_ZH_CN_TS_CONTENT;

    fn write_temp_ts(content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dtt-fill-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.ts");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn tst_export_ts_and_apply_fills_translation() {
        // TEST_ZH_CN_TS_CONTENT has one unfinished message: "England"
        let path = write_temp_ts(TEST_ZH_CN_TS_CONTENT);

        let entries = export_ts_entries(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].context, "ts::SampleContext");
        assert_eq!(entries[0].source, "England");
        assert!(!entries[0].plural);

        let resource = FillResource {
            path: path.clone(),
            kind: "ts".into(),
            entries: vec![FillEntry {
                translation: Some(FillTranslation::Singular("英格兰".into())),
                ..entries[0].clone()
            }],
        };
        apply_resource(&resource).unwrap();

        let ts = Ts::load_from_file(&path).unwrap();
        let england = ts
            .contexts
            .iter()
            .flat_map(|c| c.messages.iter())
            .find(|m| m.source == "England")
            .unwrap();
        assert!(england.translation.type_attr.is_none());
        assert_eq!(england.translation.value.as_deref(), Some("英格兰"));
    }

    #[test]
    fn tst_apply_rejects_placeholder_dropped_translation() {
        let path = write_temp_ts(r#"<?xml version="1.0" encoding="UTF-8"?>
<TS version="2.1" language="en_US">
<context>
    <name>P</name>
    <message>
        <source>Capacity %1</source>
        <translation type="unfinished"/>
    </message>
</context>
</TS>"#);
        let entries = export_ts_entries(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].placeholders, vec!["%1"]);

        let resource = FillResource {
            path: path.clone(),
            kind: "ts".into(),
            entries: vec![FillEntry {
                translation: Some(FillTranslation::Singular("指定されたコード".into())),
                ..entries[0].clone()
            }],
        };
        let err = apply_resource(&resource).unwrap_err();
        assert!(matches!(err, CmdError::PlaceholderMismatch(_, _)));

        // The file must remain untouched after a failed apply.
        let ts = Ts::load_from_file(&path).unwrap();
        let m = ts.contexts[0].messages[0].clone();
        assert!(m.translation.type_attr.is_some());
        assert!(m.translation.value.is_none());
    }

    #[test]
    fn tst_truncate_entries_honors_limit() {
        let mk = |n: i32| FillEntry {
            context: "C".into(),
            source: format!("s{}", n),
            placeholders: vec![],
            plural: false,
            plural_count: None,
            translation: None,
        };
        let entries: Vec<FillEntry> = (0..5).map(mk).collect();

        // limit less than the count -> truncate.
        let (taken, consumed) = truncate_entries(entries.clone(), 3);
        assert_eq!(consumed, 3);
        assert_eq!(taken.len(), 3);
        assert_eq!(taken[0].source, "s0");
        assert_eq!(taken[2].source, "s2");

        // limit equal/larger -> everything kept.
        let (taken, consumed) = truncate_entries(entries.clone(), 5);
        assert_eq!(consumed, 5);
        assert_eq!(taken.len(), 5);
    }
}


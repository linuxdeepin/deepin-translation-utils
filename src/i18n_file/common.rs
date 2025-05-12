// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use serde::Serialize;
use std::path::Path;
use thiserror::Error as TeError;

pub enum I18nFileKind {
    /// Qt Linguist translation file format (.ts)
    Linguist,
    /// GNU Gettext translation file format (.po)
    Gettext,
}

#[derive(TeError, Debug)]
#[error("Unknow translation file extension {ext:?}")]
pub struct UnknownI18nFileExtError {
    ext: String,
}

impl I18nFileKind {
    /// Try detecting translation file kind from given file path.
    /// 
    /// If file extension is `ts`, return Qt Linguist.
    /// If file extension is `po` or `pot`, return GNU Gettext.
    /// Otherwise return error.
    pub fn from_ext_hint(path_hint: &Path) -> Result<Self, UnknownI18nFileExtError> {
        // Get file extension and convert ot lowercase.
        let ext = path_hint.extension().map(|e| e.to_ascii_lowercase());
        let ext = match ext {
            Some(ref e) => e.to_str(),
            None => None,
        };
        // Match extension.
        match ext {
            Some("ts") => Ok(Self::Linguist),
            Some("po") | Some("pot") => Ok(Self::Gettext),
            Some(s) => Err(UnknownI18nFileExtError { ext: s.to_string() }),
            None => Err(UnknownI18nFileExtError { ext: String::new() }),
        }
    }
}

/// Universal message statistics infomations shared by all supported i18n file types.
#[derive(Debug, Default, Serialize, PartialEq)]
pub struct MessageStats {
    /// The source text has been translated.
    /// 
    /// For Qt Linguist TS file, all entries without type attribute should be grouped into this field.
    /// For GNU Gettext PO file, all translated entries should be grouped into this field.
    pub finished: u64,
    /// The source text has not been translated.
    /// 
    /// For Qt Linguist TS file, entries with "Unfinished" type should be grouped into this field.
    /// For GNU Gettext PO file, any other entries (not translated and not fuzzy) should be grouped into this field.
    pub unfinished: u64,
    /// The source text of this entry no longer exists.
    /// 
    /// It is basically same as "obsolete".
    /// The only reason why split them is to keep they are different in Qt scope.
    /// 
    /// For Qt Linguist TS file, entries with "Vanished" type should be grouped into this field.
    /// For GNU Gettext PO file, no entry should be grouped into this.
    pub vanished: u64,
    /// The source text of this entry no longer exists.
    /// 
    /// It is basically same as "vanished".
    /// The only reason why split them is to keep they are different in Qt scope.
    /// 
    /// For Qt Linguist TS file, entries with "Obsolete" type should be grouped into this field.
    /// For GNU Gettext PO file, no entry should be grouped into this.
    pub obsolete: u64,
    /// The source text of this entry is still existing,
    /// but has slight difference with old one,
    /// so the translated text may not correct.
    /// 
    /// This usually happen when use `msgmerge` in PO file
    /// when updating old translation file with new translation template,
    /// and there is a slight difference between old entry and new entry.
    /// 
    /// For Qt Linguist TS file, no entry should be grouped into this.
    /// For GNU Gettext PO file, all "fuzzy" entries should be grouped into this.
    pub fuzzy: u64,
}

impl MessageStats {
    pub fn new() -> Self {
        MessageStats {
            finished: 0,
            unfinished: 0,
            vanished: 0,
            obsolete: 0,
            fuzzy: 0,
        }
    }

    /// The "Completeness" value shown in statistics table.
    pub fn completeness_percentage(&self) -> f64 {
        let finished = self.shown_translated();
        let unfinished = self.shown_unfinished();
        
        let total = finished + unfinished;
        if total == 0 {
            return 0.0;
        } else {
            (finished as f64 / total as f64) * 100.0
        }
    }

    /// The "Translated" value shown in statistics table.
    pub fn shown_translated(&self) -> u64 {
        self.finished
    }

    /// The "Unfinished" value shown in statistics table.
    pub fn shown_unfinished(&self) -> u64 {
        self.unfinished + self.fuzzy
    }

    /// The "obsolete" value shown in statistics table.
    pub fn shown_obsolete(&self) -> u64 {
        self.obsolete + self.vanished
    }
}

impl std::ops::AddAssign<&Self> for MessageStats {
    fn add_assign(&mut self, rhs: &Self) {
        self.finished += rhs.finished;
        self.unfinished += rhs.unfinished;
        self.vanished += rhs.vanished;
        self.obsolete += rhs.obsolete;
        self.fuzzy += rhs.fuzzy;
    }
}

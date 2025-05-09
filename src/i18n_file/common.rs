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
    ext: String
}

impl I18nFileKind {
    /// Try detecting translation file kind from given file path.
    /// 
    /// If file extension is `ts`, return Qt Linguist.
    /// If file extension is `po` or `pot`, return GNU Gettext.
    /// Otherwise return error.
    pub fn from_ext_hint(path_hint: &Path) -> Result<Self, UnknownI18nFileExtError> {
        // Get file extension and convert ot lowercase.
        let ext = path_hint
            .extension()
            .map(|e| e.to_ascii_lowercase());
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

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct MessageStats {
    pub finished: u64,
    pub unfinished: u64,
    pub vanished: u64,
    pub obsolete: u64,
}

impl MessageStats {
    pub fn completeness_percentage(&self) -> f64 {
        let total = self.finished + self.unfinished;
        if total == 0 {
            return 0.0;
        }
        (self.finished as f64 / total as f64) * 100.0
    }
}

impl std::ops::AddAssign<&Self> for MessageStats {
    fn add_assign(&mut self, rhs: &Self) {
        self.finished += rhs.finished;
        self.unfinished += rhs.unfinished;
        self.vanished += rhs.vanished;
        self.obsolete += rhs.obsolete;
    }
}

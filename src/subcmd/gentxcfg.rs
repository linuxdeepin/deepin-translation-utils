// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use std::{fs, path::PathBuf};
use thiserror::Error as TeError;
use walkdir::WalkDir;
use regex::Regex;

use crate::i18n_file::common::I18nFileKind;
use crate::transifex::yaml_file::{TransifexYaml, Filter, Settings};

#[derive(TeError, Debug)]
pub enum CmdError {
    #[error("Failed to read directory: {0}")]
    ReadDir(#[from] std::io::Error),
    #[error("Failed to serialize configuration: {0}")]
    SerializeYaml(#[from] serde_yml::Error),
    #[error("Unknown translation file type: {path:?}")]
    UnknownI18nFileType { path: PathBuf },
}

pub fn subcmd_gentxcfg(project_root: &PathBuf, format: crate::cli::TxConfigFormat, ignore_paths: Vec<String>) -> Result<(), CmdError> {
    println!("Scanning directory: {:?}", project_root);

    // Scan for all translation files in the project root directory
    let all_translation_files = scan_all_translation_files(project_root, &ignore_paths)?;

    if all_translation_files.is_empty() {
        println!("No translation files (.ts or .po) found");
        return Ok(());
    }

    // Analyze and identify source files
    let source_files = identify_source_files(project_root, &all_translation_files)?;

    if source_files.is_empty() {
        println!("No source translation files found");
        return Ok(());
    }

    println!("Found {} source translation files:", source_files.len());
    for file in &source_files {
        println!("- {:?}", file);
    }

    // Generate transifex configuration
    let tx_yaml = generate_transifex_yaml(project_root, &source_files)?;

    // Create .tx directory if it doesn't exist
    let tx_dir = project_root.join(".tx");
    if !tx_dir.exists() {
        fs::create_dir_all(&tx_dir)?;
        println!("Created .tx directory");
    }

    // Generate and save file based on format
    match format {
        crate::cli::TxConfigFormat::Yaml => {
            let output_path = tx_dir.join("transifex.yaml");
            if output_path.exists() {
                println!("Note: {:?} file already exists, not overwriting.", output_path);
                println!("You can use the following content to update the file manually:\n");
                println!("{}", serde_yml::to_string(&tx_yaml)?);
            } else {
                let yaml_content = serde_yml::to_string(&tx_yaml)?;
                fs::write(&output_path, yaml_content)?;
                println!("Generated transifex.yaml file: {}", output_path.display());
            }
        },
        crate::cli::TxConfigFormat::Txconfig => {
            let tx_config = tx_yaml.to_tx_config("".to_string(), vec![]);
            let output_path = tx_dir.join("config");
            if output_path.exists() {
                println!("Note: {:?} file already exists, not overwriting.", output_path);
                println!("You can use the following content to update the file manually:\n");
                println!("{}", tx_config.to_str());
            } else {
                let config_content = tx_config.to_str();
                fs::write(&output_path, config_content)?;
                println!("Generated .tx/config file: {}", output_path.display());
            }
        },
    }

    Ok(())
}

fn scan_all_translation_files(project_root: &PathBuf, ignore_paths: &[String]) -> Result<Vec<PathBuf>, CmdError> {
    let mut translation_files = Vec::new();

    for entry in WalkDir::new(project_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_ignore_entry(e, project_root, ignore_paths))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip directories
        if !path.is_file() {
            continue;
        }

        // Check if it's a translation file
        if let Ok(_) = I18nFileKind::from_ext_hint(path) {
            translation_files.push(path.to_path_buf());
        }
    }

    Ok(translation_files)
}

fn should_ignore_entry(entry: &walkdir::DirEntry, project_root: &PathBuf, ignore_paths: &[String]) -> bool {
    let path = entry.path();

    // Get relative path from project root
    if let Ok(relative_path) = path.strip_prefix(project_root) {
        let relative_path_str = relative_path.to_string_lossy();

        for ignore_pattern in ignore_paths {
            // Skip empty patterns
            if ignore_pattern.is_empty() {
                continue;
            }

            // Check if the relative path starts with the ignore pattern
            if relative_path_str.starts_with(ignore_pattern) {
                return true;
            }

            // Check if any component of the path matches the ignore pattern
            for component in relative_path.components() {
                if let std::path::Component::Normal(name) = component {
                    if name.to_string_lossy() == ignore_pattern.as_str() {
                        return true;
                    }
                }
            }
        }
    }

    false
}

fn identify_source_files(project_root: &PathBuf, all_files: &[PathBuf]) -> Result<Vec<PathBuf>, CmdError> {
    let mut source_files = Vec::new();
    let mut processed_patterns = std::collections::HashSet::new();

    for file_path in all_files {
        // Get relative path
        let relative_path = file_path.strip_prefix(project_root)
            .unwrap_or(file_path);

        // If the file pattern has been processed, skip
        let pattern_key = get_translation_pattern(relative_path);
        if processed_patterns.contains(&pattern_key) {
            continue;
        }

        // Check if the file should be considered a source file
        if is_likely_source_file(project_root, file_path, all_files) {
            source_files.push(file_path.clone());
            processed_patterns.insert(pattern_key);
        }
    }

    Ok(source_files)
}

fn get_translation_pattern(file_path: &std::path::Path) -> String {
    let path_str = file_path.to_string_lossy().to_string();

    // Try to detect and replace language code patterns
    let detected_langs = find_language_codes_in_path(file_path);

    for lang_code in &detected_langs {
        // Check for language code patterns in filename
        if let Some(pattern) = try_extract_pattern_from_filename(&path_str, lang_code) {
            return pattern;
        }

        // Check for language code folders in path
        if let Some(pattern) = try_extract_pattern_from_path(&path_str, lang_code) {
            return pattern;
        }
    }

    // If no language code pattern found, return original path as pattern
    path_str
}

fn try_extract_pattern_from_filename(path_str: &str, lang_code: &str) -> Option<String> {
    // Pattern 1: file_zh_CN.ext -> file_<lang>.ext
    if path_str.contains(&format!("_{}", lang_code)) {
        return Some(path_str.replace(&format!("_{}", lang_code), "_<lang>"));
    }

    // Pattern 2: file.zh_CN.ext -> file.<lang>.ext
    if path_str.contains(&format!(".{}", lang_code)) {
        return Some(path_str.replace(&format!(".{}", lang_code), ".<lang>"));
    }

    // Pattern 3: zh_CN.ext -> <lang>.ext
    if let Some(file_name) = std::path::Path::new(path_str).file_name() {
        let file_name_str = file_name.to_string_lossy();
        if file_name_str.starts_with(&format!("{}.", lang_code)) {
            let parent = std::path::Path::new(path_str).parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let ext = std::path::Path::new(path_str).extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();
            if parent.is_empty() {
                return Some(format!("<lang>.{}", ext));
            } else {
                return Some(format!("{}/<lang>.{}", parent, ext));
            }
        }
    }

    None
}

fn try_extract_pattern_from_path(path_str: &str, lang_code: &str) -> Option<String> {
    // Check for language code folders in path, e.g. /zh_CN/messages.po -> /<lang>/messages.po
    let path = std::path::Path::new(path_str);
    let components: Vec<_> = path.components().collect();

    for (i, component) in components.iter().enumerate() {
        if let std::path::Component::Normal(name) = component {
            if name.to_string_lossy() == lang_code {
                let mut new_components = components.clone();
                new_components[i] = std::path::Component::Normal(std::ffi::OsStr::new("<lang>"));
                let new_path: std::path::PathBuf = new_components.iter().collect();
                return Some(new_path.to_string_lossy().to_string());
            }
        }
    }

    None
}

fn is_likely_source_file(project_root: &PathBuf, file_path: &PathBuf, all_files: &[PathBuf]) -> bool {
    let relative_path = file_path.strip_prefix(project_root).unwrap_or(file_path);
    let filename = file_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Case 1: Filename explicitly contains English language code
    if is_english_source_file(filename) {
        return true;
    }

    // Case 2: Check if path contains language code folders
    if let Some(lang_folder) = get_language_folder_in_path(relative_path) {
        // If path contains English language code folder, this is a source file
        if is_english_language_code(&lang_folder) {
            return true;
        }
        // If path contains other language code folders, this is not a source file
        return false;
    }

    // Case 3: Filename contains obvious non-English language codes, not a source file
    if contains_non_english_language_code(filename) {
        return false;
    }

    // Case 4: Check if related translation files exist in the same directory
    // If they exist, current file might be a source file
    if has_related_translation_files(project_root, file_path, all_files) {
        return true;
    }

    // Case 5: For .ts files, if no obvious language code, default to source file
    if file_path.extension().and_then(|e| e.to_str()) == Some("ts") {
        return true;
    }

    // Case 6: For .po files, check if it matches common source file name patterns
    if file_path.extension().and_then(|e| e.to_str()) == Some("po") {
        return is_common_source_po_file(filename);
    }

    false
}

fn is_english_source_file(filename: &str) -> bool {
    filename.contains("en_US") ||
    filename.contains("_en.") ||
    filename.ends_with("_en.ts") ||
    filename.ends_with("_en.po") ||
    filename.ends_with(".en.ts") ||
    filename.ends_with(".en.po")
}

fn get_language_folder_in_path(path: &std::path::Path) -> Option<String> {
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            // Skip directory names that are file extensions
            if !is_file_extension(&name_str) && is_language_code(&name_str) {
                return Some(name_str.to_string());
            }
        }
    }
    None
}

fn is_english_language_code(lang_code: &str) -> bool {
    matches!(lang_code, "en" | "en_US" | "en_GB")
}

fn contains_non_english_language_code(filename: &str) -> bool {
    let detected_langs = find_language_codes_in_filename(filename);

    for lang_code in &detected_langs {
        // Skip English-related codes
        if is_english_language_code(lang_code) {
            continue;
        }
        return true;
    }
    false
}

fn has_related_translation_files(_project_root: &PathBuf, source_file: &PathBuf, all_files: &[PathBuf]) -> bool {
    let source_dir = source_file.parent();
    let source_name = source_file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let source_ext = source_file.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Search for related translation files in the same directory
    for file in all_files {
        if file == source_file {
            continue;
        }

        // Check if in the same directory
        if file.parent() != source_dir {
            continue;
        }

        // Check if extension is the same
        if file.extension().and_then(|e| e.to_str()).unwrap_or("") != source_ext {
            continue;
        }

        let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check if it's a related translation file (contains language code)
        if file_name.starts_with(&format!("{}_", source_name)) {
            let suffix = &file_name[source_name.len() + 1..];
            if suffix.starts_with(source_ext) {
                continue; // This is just the extension
            }
            // Check if suffix is a language code
            let lang_part = suffix.split('.').next().unwrap_or("");
            if is_language_code(lang_part) {
                return true;
            }
        }
    }

    false
}

fn is_common_source_po_file(filename: &str) -> bool {
    filename.starts_with("messages") ||
    filename.starts_with("strings") ||
    filename.starts_with("template") ||
    filename.contains("_template") ||
    filename == "default.po" ||
    filename == "base.po"
}

/// Check if a string matches ISO 639/3166 language code format
/// Supports formats: xx (ISO 639 language) or xx_YY (language_REGION)
fn is_language_code(code: &str) -> bool {
    // Regex for ISO 639/3166 format: xx or xx_YY where:
    // - xx is 2 lowercase letters (ISO 639 language code), note that some files
    //      use 3 letters language codes (kab, ast), so we use 2-3 letters for now.
    // - YY is 2 or 3 uppercase letters (ISO 3166 country/region code)
    let lang_regex = Regex::new(r"^[a-z]{2,3}(_[A-Z]{2,3})?$").unwrap();
    lang_regex.is_match(code)
}

/// Find all language codes in a file path (both filename and directory components)
fn find_language_codes_in_path(path: &std::path::Path) -> Vec<String> {
    let mut codes = Vec::new();

    // Check filename (excluding extension)
    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
        codes.extend(find_language_codes_in_filename(filename));
    }

    // Check directory components (but skip if they match file extensions)
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            // Skip directory names that are file extensions
            if !is_file_extension(&name_str) && is_language_code(&name_str) {
                codes.push(name_str.to_string());
            }
        }
    }

    // Remove duplicates
    codes.sort();
    codes.dedup();
    codes
}

/// Check if a string looks like a file extension
fn is_file_extension(s: &str) -> bool {
    // Common file extensions that we want to avoid treating as language codes
    let extensions = [
        "po", "pot", "ts", "js", "py", "rs", "go", "sh", "rb", "md",
        "txt", "xml", "json", "yaml", "yml", "toml", "ini", "cfg",
        "html", "css", "scss", "less", "vue", "jsx", "tsx",
        "c", "cpp", "h", "hpp", "cs", "java", "kt", "php",
        "sql", "db", "sqlite", "log", "tmp", "bak", "old"
    ];
    extensions.contains(&s)
}

/// Find language codes in a filename using various patterns
fn find_language_codes_in_filename(filename: &str) -> Vec<String> {
    let mut codes = Vec::new();

    // Get the file stem (filename without extension) to avoid matching extensions
    let file_stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    // Pattern 1: filename_xx or filename_xx_YY (underscore separated)
    let underscore_regex = Regex::new(r"_([a-z]{2}(?:_[A-Z]{2,3})?)$").unwrap();
    for cap in underscore_regex.captures_iter(file_stem) {
        if let Some(code) = cap.get(1) {
            codes.push(code.as_str().to_string());
        }
    }

    // Pattern 2: filename.xx or filename.xx_YY (dot separated, but not at the end)
    let dot_regex = Regex::new(r"\.([a-z]{2}(?:_[A-Z]{2,3})?)(?:\.|$)").unwrap();
    for cap in dot_regex.captures_iter(file_stem) {
        if let Some(code) = cap.get(1) {
            codes.push(code.as_str().to_string());
        }
    }

    // Pattern 3: xx or xx_YY (filename starts with language code)
    let start_regex = Regex::new(r"^([a-z]{2}(?:_[A-Z]{2,3})?)(?:\.|_|$)").unwrap();
    if let Some(cap) = start_regex.captures(file_stem) {
        if let Some(code) = cap.get(1) {
            let code_str = code.as_str();
            // Only add if it's not a file extension
            if !is_file_extension(code_str) {
                codes.push(code_str.to_string());
            }
        }
    }

    // Remove duplicates
    codes.sort();
    codes.dedup();
    codes
}

fn generate_transifex_yaml(project_root: &PathBuf, translation_files: &[PathBuf]) -> Result<TransifexYaml, CmdError> {
    let mut filters = Vec::new();

    for file_path in translation_files {
        // Get relative path
        let relative_path = file_path.strip_prefix(project_root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // Determine file format
        let file_kind = I18nFileKind::from_ext_hint(file_path)
            .map_err(|_| CmdError::UnknownI18nFileType { path: file_path.clone() })?;

        let file_format = match file_kind {
            I18nFileKind::Linguist => "QT",
            I18nFileKind::Gettext => "PO",
        };

        // Generate translation file expression
        let translation_expression = generate_translation_expression(&relative_path);

        let filter = Filter {
            type_attr: "file".to_string(),
            source: relative_path,
            format: file_format.to_string(),
            source_lang: "en_US".to_string(),
            target_pattern: translation_expression,
        };

        filters.push(filter);
    }

    Ok(TransifexYaml {
        filters,
        settings: Settings {
            branch_template: "transifex_update_<br_unique_id>".to_string(),
        },
    })
}

fn generate_translation_expression(source_file: &str) -> String {
    let source_path = std::path::Path::new(source_file);

    // First try to detect and replace existing English language code patterns
    if source_file.contains("_en_US") {
        return source_file.replace("_en_US", "_<lang>");
    } else if source_file.contains("_en.") {
        return source_file.replace("_en.", "_<lang>.");
    } else if source_file.contains(".en.") {
        return source_file.replace(".en.", ".<lang>.");
    } else if source_file.ends_with("_en.ts") {
        return source_file.replace("_en.ts", "_<lang>.ts");
    } else if source_file.ends_with("_en.po") {
        return source_file.replace("_en.po", "_<lang>.po");
    } else if source_file.ends_with(".en.ts") {
        return source_file.replace(".en.ts", ".<lang>.ts");
    } else if source_file.ends_with(".en.po") {
        return source_file.replace(".en.po", ".<lang>.po");
    }

    // If source file path has a folder named "en" or similar, replace that folder
    let components: Vec<_> = source_path.components().collect();
    for (i, component) in components.iter().enumerate() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            if is_english_language_code(&name_str) {
                let mut new_components = components.clone();
                new_components[i] = std::path::Component::Normal(std::ffi::OsStr::new("<lang>"));
                let new_path: std::path::PathBuf = new_components.iter().collect();
                return new_path.to_string_lossy().to_string();
            }
        }
    }

    // Default case: add language code before file extension
    if let Some(dot_pos) = source_file.rfind('.') {
        let (name, ext) = source_file.split_at(dot_pos);
        format!("{}_<lang>{}", name, ext)
    } else {
        format!("{}_<lang>", source_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_code_detection() {
        // Test ISO 639/3166 language code format validation
        assert!(is_language_code("en"));
        assert!(is_language_code("zh"));
        assert!(is_language_code("en_US"));
        assert!(is_language_code("zh_CN"));
        assert!(is_language_code("zh_TW"));
        assert!(is_language_code("pt_BR"));
        assert!(is_language_code("fr"));
        assert!(is_language_code("de"));
        assert!(is_language_code("ja"));
        // Test 3-letter "language codes"
        assert!(is_language_code("kab"));
        assert!(is_language_code("ast"));

        // Test invalid formats
        assert!(!is_language_code("english"));
        assert!(!is_language_code("EN"));
        assert!(!is_language_code("en_us"));
        assert!(!is_language_code("zh_cn"));
        assert!(!is_language_code(""));

        // Test valid language codes that might look like file extensions
        assert!(is_language_code("so")); // Somali language
        assert!(is_language_code("in")); // Indonesian language (deprecated, but valid)
        assert!(is_language_code("os")); // Ossetian language
        assert!(is_language_code("io")); // Ido language

        // Test file extension detection
        assert!(is_file_extension("po"));
        assert!(is_file_extension("ts"));
        assert!(is_file_extension("js"));
        assert!(is_file_extension("py"));
        assert!(is_file_extension("rs"));
        assert!(!is_file_extension("zh_CN"));
        assert!(!is_file_extension("en_US"));

        // Test English source file detection
        assert!(is_english_source_file("messages_en_US.po"));
        assert!(is_english_source_file("strings_en.ts"));
        assert!(is_english_source_file("app.en.ts"));
        assert!(is_english_source_file("dialog.en.po"));

        // Test finding language codes in filename (should exclude file extensions)
        assert_eq!(find_language_codes_in_filename("app_zh_CN.ts"), vec!["zh_CN"]);
        assert_eq!(find_language_codes_in_filename("messages.ja.po"), vec!["ja"]);
        assert_eq!(find_language_codes_in_filename("fr.po"), vec!["fr"]);
        assert_eq!(find_language_codes_in_filename("app.ts"), Vec::<String>::new());
        assert_eq!(find_language_codes_in_filename("strings_so.po"), vec!["so"]); // Somali language

        // Test that file extensions are not detected as language codes
        assert_eq!(find_language_codes_in_filename("po.po"), Vec::<String>::new()); // 'po' should be filtered out as extension
        assert_eq!(find_language_codes_in_filename("ts.ts"), Vec::<String>::new()); // 'ts' should be filtered out as extension

        // Test non-English language code detection
        assert!(contains_non_english_language_code("app_zh_CN.ts"));
        assert!(contains_non_english_language_code("messages_zh_TW.po"));
        assert!(contains_non_english_language_code("zh_CN.po"));
        assert!(contains_non_english_language_code("ja.po"));
        assert!(contains_non_english_language_code("ko_KR.ts"));
        assert!(!contains_non_english_language_code("app.ts"));
        assert!(!contains_non_english_language_code("messages_en.po"));

        // Test language code folder detection in path
        assert_eq!(
            get_language_folder_in_path(std::path::Path::new("translations/zh_CN/messages.po")),
            Some("zh_CN".to_string())
        );
        assert_eq!(
            get_language_folder_in_path(std::path::Path::new("locales/ja/strings.ts")),
            Some("ja".to_string())
        );
        assert_eq!(
            get_language_folder_in_path(std::path::Path::new("translations/messages.po")),
            None
        );
        assert_eq!(
            get_language_folder_in_path(std::path::Path::new("po/en/messages.po")),
            Some("en".to_string())
        );

        // Test English language code detection
        assert!(is_english_language_code("en"));
        assert!(is_english_language_code("en_US"));
        assert!(is_english_language_code("en_GB"));
        assert!(!is_english_language_code("zh_CN"));
        assert!(!is_english_language_code("ja"));

        // Test common source file detection
        assert!(is_common_source_po_file("messages.po"));
        assert!(is_common_source_po_file("strings.po"));
        assert!(is_common_source_po_file("template.po"));
        assert!(is_common_source_po_file("default.po"));
        assert!(!is_common_source_po_file("zh_CN.po"));
    }

    #[test]
    fn test_generate_translation_expression() {
        // Test English language code replacement
        assert_eq!(
            generate_translation_expression("app_en_US.ts"),
            "app_<lang>.ts"
        );
        assert_eq!(
            generate_translation_expression("messages_en.po"),
            "messages_<lang>.po"
        );
        assert_eq!(
            generate_translation_expression("dialog.en.ts"),
            "dialog.<lang>.ts"
        );

        // Test files without language codes
        assert_eq!(
            generate_translation_expression("strings.ts"),
            "strings_<lang>.ts"
        );
        assert_eq!(
            generate_translation_expression("messages.po"),
            "messages_<lang>.po"
        );

        // Test paths containing language code folders
        assert_eq!(
            generate_translation_expression("locales/en/messages.po"),
            "locales/<lang>/messages.po"
        );
        assert_eq!(
            generate_translation_expression("po/en_US/strings.po"),
            "po/<lang>/strings.po"
        );
    }

    #[test]
    fn test_get_translation_pattern() {
        // Test language code pattern extraction from filename
        assert_eq!(
            get_translation_pattern(std::path::Path::new("app_zh_CN.ts")),
            "app_<lang>.ts"
        );
        assert_eq!(
            get_translation_pattern(std::path::Path::new("messages.zh_TW.po")),
            "messages.<lang>.po"
        );
        assert_eq!(
            get_translation_pattern(std::path::Path::new("zh_CN.po")),
            "<lang>.po"
        );

        // Test language code folder pattern extraction from path
        assert_eq!(
            get_translation_pattern(std::path::Path::new("locales/zh_CN/messages.po")),
            "locales/<lang>/messages.po"
        );

        // Test files without language codes
        assert_eq!(
            get_translation_pattern(std::path::Path::new("app.ts")),
            "app.ts"
        );
    }
}
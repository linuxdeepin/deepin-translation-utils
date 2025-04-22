# deepin-translation-utils

A commandline tool to help you work with Qt Linguist-based translation files that are used in deepin's workflow.

This program currently supports the following features:

- Converts Chinese texts in Qt linguist TS file among regional variants.

## Usage

Please consult `deepin-translation-utils --help`.

## Dependencies

Please consult `Cargo.toml`.

### Note:

`quick_xml` (with `serialize` feature enabled) instead of `serde_xml_rs` is used to parse and write XML files because of there are [known bugs](https://github.com/RReverser/serde-xml-rs/issues/186) that hasn't be fixed which also appears in this program. Their derive macros syntax are **not compatible**.
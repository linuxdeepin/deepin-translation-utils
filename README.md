# deepin-translation-utils

A commandline tool to help you work with Qt Linguist-based and Gettext-based translation files, and Transifex platform-related configurations that are used in deepin's workflow.

This program currently supports the following features:

- Converts Chinese texts in Qt linguist TS file or GNU Gettext PO file among regional variants.
- Prints translation statistics of the provided project.
- Generates `.tx/config` based on Transifex GitHub integration `transifex.yaml` config file and Transifex API.
  - Transifex API is used to look up and match the resource slug.
  - Local cache can be used without making API request if the resource info data is already fetched previously.
- Generates Transifex GitHub integration `transifex.yaml` based on `.tx/config`.

## Usage

Please consult `deepin-translation-utils --help`.

## Dependencies

Please consult `Cargo.toml`.

### Note:

`quick_xml` (with `serialize` feature enabled) instead of `serde_xml_rs` is used to parse and write XML files because of there are [known bugs](https://github.com/RReverser/serde-xml-rs/issues/186) that hasn't be fixed which also appears in this program. Their derive macros syntax are **not compatible**.
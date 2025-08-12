# deepin-translation-utils

A commandline tool to help you work with Qt Linguist-based and Gettext-based translation files, and Transifex platform-related configurations that are used in deepin's workflow.

This program currently supports the following features:

- Converts Chinese texts in Qt linguist TS file or GNU Gettext PO file among regional variants.
- Prints translation statistics of the provided project.
  - It offers an [online translation-stats-viewer page](https://linuxdeepin.github.io/deepin-translation-utils/translation-stats-viewer.html) to help display stats exported to `json` file.
- Generates `.tx/config` based on Transifex GitHub integration `transifex.yaml` config file and Transifex API.
  - Transifex API is used to look up and match the resource slug.
  - Local cache can be used without making API request if the resource info data is already fetched previously.
- Generates Transifex GitHub integration `transifex.yaml` based on `.tx/config`.
- Generate a single `.tx/config` contains all linked resources under the given Transifex organization.
- Generate `.tx/transifex.yaml` or `.tx/config` based on the (`.po` abd `.ts`) translation files inside the given source repo.

## Usage

Please consult `deepin-translation-utils --help`.

## Dependencies

Please consult `Cargo.toml`.

### Note:

- Don't blindly pull translation resources after using the `monotxconfig` subcommand to generate `.tx/config` unless you are absolutely sure what you're doing. Pulling all translation resources directly from Transifex is a very slow process.
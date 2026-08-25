# deepin-translation-utils

A commandline tool to help you work with Qt Linguist-based and Gettext-based translation files, and Transifex platform-related configurations that are used in deepin's workflow.

This program currently supports the following features:

- Converts Chinese texts in Qt linguist TS file or GNU Gettext PO file among regional variants.
- Prints translation statistics of the provided project.
- Generates `.tx/config` based on Transifex GitHub integration `transifex.yaml` config file and Transifex API.
  - Transifex API is used to look up and match the resource slug.
  - Local cache can be used without making API request if the resource info data is already fetched previously.
- Generates Transifex GitHub integration `transifex.yaml` based on `.tx/config`.
- Generate a single `.tx/config` contains all linked resources under the given Transifex organization.
- Generate `.tx/transifex.yaml` or `.tx/config` based on the (`.po` abd `.ts`) translation files inside the given source repo.
- Two-step translation back-fill pipeline driven by an external provider:
  - `export-translation <project> <lang> [--limit N]` prints a JSON document listing every unfinished `.ts`/`.po` message (source, context, placeholders, plural-form count). `--limit` restricts the output to the first N messages for batch filling.
  - `fill-translation <project> <json>` reads the document back once the `translation` fields are filled, validates placeholders and plural-form counts, and writes them into the files. Entries left empty are skipped, so the same document can be applied in several batches.

## Install

### Via `cargo-binstall` (suggested)

If you have [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) installed, you can use it to install this program.

```bash
cargo binstall deepin-translation-utils
```

### Via `cargo install`

```bash
cargo install deepin-translation-utils
```

### Manual download

Get the latest release at [GitHub Release page](https://github.com/linuxdeepin/deepin-translation-utils/releases/latest), download it, extract it, and put it in your `$PATH` (usually we suggest to use `~/.local/bin/`).

## Usage

Please consult `deepin-translation-utils --help`.

## Dependencies

Please consult `Cargo.toml`.

### Note:

- Don't blindly pull translation resources after using the `monotxconfig` subcommand to generate `.tx/config` unless you are absolutely sure what you're doing. Pulling all translation resources directly from Transifex is a very slow process.
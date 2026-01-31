# deepin-translation-utils

一个用于帮助你处理 deepin 项目中涉及到的基于 Qt Linguist 与 GNU Gettext 的翻译文件，以及 Transifex 平台配置文件的相关工作的命令行工具。

此工具目前支持以下功能：

- 将 Qt Linguist TS 或 GNU Gettext PO 文件中的中文文本转换为不同的区域变体（简转繁等）。
- 统计并展示所提供的项目的翻译完成度。
- 根据 Transifex GitHub 集成配置文件 `transifex.yaml` 以及 Transifex API 生成 `.tx/config`。
  - Transifex API 用以查询和关联资源对应的 slug。
  - 如果对应的资源信息本地已有缓存，也可以使用对应的缓存信息而不进行 API 请求。
- 根据 `.tx/config` 生成 Transifex GitHub 集成配置文件 `transifex.yaml`。
- 根据给定的 Transifex 组织，生成一个包含所有关联资源的 `.tx/config`。
- 根据所给定仓库内的(`.po` 与 `.ts`)翻译文件，生成 `.tx/transifex.yaml` 或 `.tx/config` 配置文件。

## 安装

### 使用 `cargo-binstall` (建议)

如果你有 [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) 安装，你可以使用它来安装此程序。

```bash
cargo binstall deepin-translation-utils
```

### 使用 `cargo install`

```bash
cargo install deepin-translation-utils
```

### 手动下载

你可以在 [GitHub Release 页面](https://github.com/linuxdeepin/deepin-translation-utils/releases/latest) 下载最新版本，解压后将其放入你的 `$PATH` 中（通常我们建议使用 `~/.local/bin/`）。

## 用法

请参阅 `deepin-translation-utils --help`。

## 依赖

请参阅 `Cargo.toml`。

## 注意：

- 除非你绝对确定你在做什么，否则不要在使用 `monotxconfig` 子命令生成 `.tx/config` 后盲目拉取翻译资源。直接从 Transifex 拉取所有翻译资源的过程会特别慢。
# deepin-translation-utils

一个用于帮助你处理 deepin 项目中涉及到的基于 Qt Linguist 与 GNU Gettext 的翻译文件，以及 Transifex 平台配置文件的相关工作的命令行工具。

此工具目前支持以下功能：

- 将 Qt Linguist TS 或 GNU Gettext PO 文件中的中文文本转换为不同的区域变体（简转繁等）。
- 统计并展示所提供的项目的翻译完成度。
- 根据 Transifex GitHub 集成配置文件 `transifex.yaml` 以及 Transifex API 生成 `.tx/config`。
  - Transifex API 用以查询和关联资源对应的 slug。
  - 如果对应的资源信息本地已有缓存，也可以使用对应的缓存信息而不进行 API 请求。
- 根据 `.tx/config` 生成 Transifex GitHub 集成配置文件 `transifex.yaml`。

## 用法

请参阅 `deepin-translation-utils --help`。

## 依赖

请参阅 `Cargo.toml`。

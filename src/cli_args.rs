// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

use std::path::PathBuf;
use clap::{Parser, Subcommand};


#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
#[command(
    version = env!("GIT_DESCRIBE_OR_CARGO_PKG_VERSION"),
    about = "A commandline tool to help you work with translation files and Transifex configurations."
)]
pub enum Commands {
    #[command(name = "zhconv")]
    #[command(
        about = "Converts Chinese texts in Qt Linguist or GNU Gettext file among regional variants",
        long_about = "Converts given Qt Linguist (.ts) file or GNU Gettext (.po) file among traditional/simplified scripts or regional variants.\n\n\
            Converted files are written to the same directory as the original file with the same name but with different language code suffix to the file name.",
    )]
    ZhConv {
        #[arg(short, long, default_value = "zh_CN")]
        source_language: String,
        #[arg(short, long, default_value = "zh_HK,zh_TW", value_delimiter = ',')]
        target_languages: Vec<String>,
        linguist_ts_file: PathBuf,
    },

    #[command(name = "zhconv-plain")]
    #[command(
        about = "Converts given Chinese texts among regional variants",
        long_about = "Converts given text among traditional/simplified scripts or regional variants.\n\n\
            Converted texts are printed to stdout, splitted by new line.",
    )]
    ZhConvPlain {
        #[arg(short, long, default_value = "zh_HK,zh_TW", value_delimiter = ',')]
        target_languages: Vec<String>,
        content: String,
    },

    #[command(name = "statistics")]
    #[command(
        about = "Prints translation statistics of the provided project",
        long_about = "Prints translation statistics of the provided project according to transifex.yaml or .tx/config file.\n\n\
            Only Qt Linguist-based resources are processed, other resources like PO-based ones are ignored.",
    )]
    Statistics {
        project_root: PathBuf,
        #[clap(short, long, default_value_t, value_enum)]
        format: crate::subcmd_statistics::StatsFormat,
        #[clap(short, long, default_value_t, value_enum)]
        sort_by: crate::subcmd_statistics::StatsSortBy,
    },
    #[command(name = "yaml2txconfig")]
    #[command(
        about = "Generate .tx/config based on transifex.yaml",
        long_about = "Generate .tx/config based on transifex.yaml\n\n\
            Missing resource slugs will be looked-up via API or local cached data.",
    )]
    Yaml2TxConfig {
        project_root: PathBuf,
        /// Force to fetch the resource slugs via Transifex REST API, and update local cache.
        #[clap(short, long, action = clap::ArgAction::SetTrue, default_value_t = false)]
        force_online: bool,
        /// GitHub repository name in owner/repo format. e.g. linuxdeepin/dde-control-center
        #[arg(short, long)]
        github_repository: Option<String>,
        /// organization slug of the project on Transifex platform
        #[arg(short, long, default_value = "linuxdeepin")]
        organization_slug: String,
        /// project slug of the project on Transifex platform.
        /// If not provided, it will lookup all projects under the organization slug.
        #[arg(short, long, default_value = None)]
        project_slug: Option<String>,
    },
    #[command(name = "txconfig2yaml")]
    #[command(
        about = "Generate transifex.yaml based on .tx/config",
    )]
    TxConfig2Yaml {
        project_root: PathBuf,
    },
}
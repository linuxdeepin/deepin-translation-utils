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
pub enum Commands {
    #[command(name = "zhconv")]
    #[command(
        about = "Converts Chinese texts in Qt linguist TS file among regional variants",
        long_about = "Converts given Qt linguist TS file among traditional/simplified scripts or regional variants.\n\n\
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
    }
}
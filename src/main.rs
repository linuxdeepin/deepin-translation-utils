// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

mod cli_args;
mod linguist_file;
mod transifex_yaml_file;
mod tx_config_file;
mod transifex_rest_api;
mod subcmd_zhconv;
mod subcmd_statistics;
mod subcmd_yaml2txconfig;
mod subcmd_txconfig2yaml;

use crate::cli_args::*;
use crate::subcmd_zhconv::*;
use crate::subcmd_statistics::*;
use crate::subcmd_yaml2txconfig::*;
use crate::subcmd_txconfig2yaml::*;

use clap::Parser;

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::ZhConv { source_language, target_languages, linguist_ts_file } => {
            subcmd_zhconv(source_language, target_languages, linguist_ts_file).unwrap_or_else(|err| {
                eprintln!("\x1B[31m{0}\x1B[0m", err);
                std::process::exit(1);
            });
        },
        Commands::ZhConvPlain { target_languages, content } => {
            subcmd_zhconv_plain(target_languages, content).unwrap_or_else(|err| {
                eprintln!("\x1B[31m{0}\x1B[0m", err);
                std::process::exit(1);
            });
        },
        Commands::Statistics { project_root, format, sort_by} => {
            subcmd_statistics(&project_root, format, sort_by).unwrap_or_else(|err| {
                eprintln!("\x1B[31m{0}\x1B[0m", err);
                std::process::exit(1);
            });
        },
        Commands::Yaml2TxConfig { project_root, force_online, github_repository, organization_slug, project_slug } => {
            subcmd_yaml2txconfig(&project_root, force_online, github_repository, organization_slug, project_slug).unwrap_or_else(|err| {
                eprintln!("\x1B[31m{0}\x1B[0m", err);
                std::process::exit(1);
            })
        },
        Commands::TxConfig2Yaml { project_root } => {
            subcmd_txconfig2yaml(&project_root).unwrap_or_else(|err| {
                eprintln!("\x1B[31m{0}\x1B[0m", err);
                std::process::exit(1);
            })
        }
    }
}

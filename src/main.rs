// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

mod cli_args;
mod linguist_file;
mod transifex_yaml_file;
mod subcmd_zhconv;
mod subcmd_statistics;

use crate::cli_args::*;
use crate::subcmd_zhconv::*;
use crate::subcmd_statistics::*;

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
        }
    }
}

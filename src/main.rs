// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

fn main() {
    deepin_translation_utils::cli::execute().unwrap_or_else(|err| {
        eprintln!("\x1B[31m{0}\x1B[0m", err);
        std::process::exit(1);
    });
}

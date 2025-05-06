use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=.git/HEAD");
    let output = Command::new("git").args(&["describe", "--tags", "--long"]).output();
    let git_describe_or_fallback = match output {
        Ok(output) => {
            let rev = String::from_utf8(output.stdout).unwrap_or("unknown".to_owned());
            if rev.trim().is_empty() {
                env!("CARGO_PKG_VERSION").to_owned()
            } else {
                format!("{}", rev.trim())
            }
        },
        Err(_) => env!("CARGO_PKG_VERSION").to_owned(),
    };
    println!("cargo:rustc-env=GIT_DESCRIBE_OR_CARGO_PKG_VERSION={}", git_describe_or_fallback);
}
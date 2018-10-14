extern crate foreman;

use std::process::Command;
use std::process::Output;

fn main() {
    set_git_ver_env_var();
}

fn set_git_ver_env_var() {
    let git_ver = Command::new("git")
        .args(&[
            "describe",
            "--tags",
            "--first-parent",
            "--always",
            "--dirty",
            "--broken",
        ]).output();

    let git_ver = match git_ver {
        Ok(Output {
            ref status,
            ref stdout,
            ..
        })
            if status.success() =>
        {
            let v = String::from_utf8_lossy(stdout);
            eprintln!("Detected version from Git repository: {}", v);
            v
        }
        o => {
            foreman::warning(&format!(
                "Error running `git describe`: {}",
                match o {
                    Ok(Output { ref stderr, .. }) => String::from_utf8_lossy(stderr).to_string(),
                    Err(e) => e.to_string(),
                }
            ));
            "".into()
        }
    };

    foreman::env_var("IRC_BOT_RS_GIT_VERSION", &git_ver);
}

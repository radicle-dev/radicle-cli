use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .arg("rev-parse")
        .arg("--short")
        .arg("HEAD")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| String::from("unknown"));

    println!("cargo:rustc-env=GIT_HEAD={}", hash);
    println!("cargo:rustc-rerun-if-changed=.git/HEAD");
}

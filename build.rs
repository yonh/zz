fn main() {
    // Try to get git short SHA for converter version
    let converter_version = match std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8(output.stdout).unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
        }
        _ => env!("CARGO_PKG_VERSION").to_string(),
    };

    println!("cargo:rustc-env=CONVERTER_VERSION={}", converter_version.trim());
}

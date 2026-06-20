use super::types::{BrowserRuntimeArchiveFormat, BrowserRuntimeSpec};

impl BrowserRuntimeSpec {
    #[cfg(test)]
    pub(super) fn for_test(platform: &str, version: &str, expected_archive_sha256: &str) -> Self {
        Self {
            platform: platform.to_string(),
            version: version.to_string(),
            download_url: format!("https://example.test/{platform}/{version}.zip"),
            expected_archive_sha256: expected_archive_sha256.to_string(),
            archive_format: BrowserRuntimeArchiveFormat::Zip,
            archive_root_dir: format!("chrome-{platform}"),
            relative_executable_path: "chrome".to_string(),
        }
    }
}

pub fn current_runtime_spec() -> Option<BrowserRuntimeSpec> {
    match current_platform().as_str() {
        "mac-arm64" => Some(BrowserRuntimeSpec {
            platform: "mac-arm64".to_string(),
            version: "149.0.7827.55".to_string(),
            download_url:
                "https://storage.googleapis.com/chrome-for-testing-public/149.0.7827.55/mac-arm64/chrome-mac-arm64.zip"
                    .to_string(),
            expected_archive_sha256:
                "311211b54c429245e2cec0314ee1e314085e9c00350215b95e1a879350786630"
                    .to_string(),
            archive_format: BrowserRuntimeArchiveFormat::Zip,
            archive_root_dir: "chrome-mac-arm64".to_string(),
            relative_executable_path:
                "Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
                    .to_string(),
        }),
        _ => None,
    }
}

pub fn current_platform() -> String {
    let os = std::env::consts::OS;
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    };

    match os {
        "macos" => format!("mac-{arch}"),
        other => format!("{other}-{arch}"),
    }
}

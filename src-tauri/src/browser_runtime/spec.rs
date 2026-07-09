use super::types::{BrowserRuntimeArchiveFormat, BrowserRuntimeSpec};

const CHROME_FOR_TESTING_VERSION: &str = "149.0.7827.55";
const CHROME_FOR_TESTING_BASE_URL: &str =
    "https://storage.googleapis.com/chrome-for-testing-public";
const MACOS_CHROME_EXECUTABLE: &str =
    "Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing";

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
    runtime_spec_for_platform(&current_platform())
}

fn runtime_spec_for_platform(platform: &str) -> Option<BrowserRuntimeSpec> {
    match platform {
        "mac-arm64" => Some(chrome_for_testing_spec(
            platform,
            "mac-arm64",
            "311211b54c429245e2cec0314ee1e314085e9c00350215b95e1a879350786630",
            MACOS_CHROME_EXECUTABLE,
        )),
        "mac-x64" => Some(chrome_for_testing_spec(
            platform,
            "mac-x64",
            "4fff3b1bff4ab5acab495438d501fd56ecd326fc2e18670858930386dca864e6",
            MACOS_CHROME_EXECUTABLE,
        )),
        "linux-x64" => Some(chrome_for_testing_spec(
            platform,
            "linux64",
            "13113b963ac22fffdad898a677591028e4397c46c1daa9e61811258eed6e35b5",
            "chrome",
        )),
        "windows-x64" => Some(chrome_for_testing_spec(
            platform,
            "win64",
            "ebc0c2b75e2ea98151a7f18ff47037bfcbab44a8660e79b9ffa6520f9b7607ab",
            "chrome.exe",
        )),
        _ => None,
    }
}

fn chrome_for_testing_spec(
    platform: &str,
    chrome_for_testing_platform: &str,
    expected_archive_sha256: &str,
    relative_executable_path: &str,
) -> BrowserRuntimeSpec {
    BrowserRuntimeSpec {
        platform: platform.to_string(),
        version: CHROME_FOR_TESTING_VERSION.to_string(),
        download_url: format!(
            "{CHROME_FOR_TESTING_BASE_URL}/{CHROME_FOR_TESTING_VERSION}/{chrome_for_testing_platform}/chrome-{chrome_for_testing_platform}.zip"
        ),
        expected_archive_sha256: expected_archive_sha256.to_string(),
        archive_format: BrowserRuntimeArchiveFormat::Zip,
        archive_root_dir: format!("chrome-{chrome_for_testing_platform}"),
        relative_executable_path: relative_executable_path.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_spec_supports_desktop_release_platforms() {
        let supported = [
            ("mac-arm64", "mac-arm64", MACOS_CHROME_EXECUTABLE),
            ("mac-x64", "mac-x64", MACOS_CHROME_EXECUTABLE),
            ("linux-x64", "linux64", "chrome"),
            ("windows-x64", "win64", "chrome.exe"),
        ];

        for (platform, chrome_for_testing_platform, executable_path) in supported {
            let spec = runtime_spec_for_platform(platform).expect("platform should be supported");

            assert_eq!(spec.platform, platform);
            assert_eq!(spec.version, CHROME_FOR_TESTING_VERSION);
            assert_eq!(
                spec.download_url,
                format!(
                    "{CHROME_FOR_TESTING_BASE_URL}/{CHROME_FOR_TESTING_VERSION}/{chrome_for_testing_platform}/chrome-{chrome_for_testing_platform}.zip"
                )
            );
            assert_eq!(
                spec.archive_root_dir,
                format!("chrome-{chrome_for_testing_platform}")
            );
            assert_eq!(spec.relative_executable_path, executable_path);
            assert_eq!(spec.archive_format, BrowserRuntimeArchiveFormat::Zip);
            assert!(!spec.expected_archive_sha256.is_empty());
        }
    }

    #[test]
    fn runtime_spec_rejects_unsupported_platforms() {
        assert_eq!(runtime_spec_for_platform("linux-arm64"), None);
        assert_eq!(runtime_spec_for_platform("windows-arm64"), None);
    }
}

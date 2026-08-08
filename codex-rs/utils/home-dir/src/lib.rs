use codex_utils_absolute_path::AbsolutePathBuf;
use dirs::home_dir;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

const CODEX_HOME_ENV: &str = "CODEX_HOME";
const CODEX_HOME_DIR: &str = ".codex";
const UNIVERS_CODE_BINARY: &str = "univers-code";
const UNIVERS_CODE_HOME_ENV: &str = "UNIVERS_CODE_HOME";
const UNIVERS_CODE_HOME_DIR: &str = ".univers-code";

/// Returns the path to the Codex configuration directory, which can be
/// specified by the `CODEX_HOME` environment variable. If not set, defaults to
/// `~/.codex`. The `univers-code` binary instead uses `UNIVERS_CODE_HOME`, then
/// falls back to `CODEX_HOME`, and finally defaults to `~/.univers-code`.
///
/// - An explicitly configured home must exist and be a directory. The value
///   will be canonicalized and this function will Err otherwise.
/// - A default home path is returned without verifying that it exists.
pub fn find_codex_home() -> std::io::Result<AbsolutePathBuf> {
    let is_univers_code = std::env::current_exe()
        .ok()
        .as_deref()
        .is_some_and(is_univers_code_executable);
    let univers_code_home_env = is_univers_code
        .then(|| non_empty_env(UNIVERS_CODE_HOME_ENV))
        .flatten();
    let codex_home_env = non_empty_env(CODEX_HOME_ENV);

    if let Some(value) = univers_code_home_env.as_deref() {
        return find_home_from_env(Some((UNIVERS_CODE_HOME_ENV, value)), UNIVERS_CODE_HOME_DIR);
    }

    let default_home_dir = if is_univers_code {
        UNIVERS_CODE_HOME_DIR
    } else {
        CODEX_HOME_DIR
    };
    find_home_from_env(
        codex_home_env
            .as_deref()
            .map(|value| (CODEX_HOME_ENV, value)),
        default_home_dir,
    )
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn is_univers_code_executable(path: &Path) -> bool {
    path.file_stem() == Some(OsStr::new(UNIVERS_CODE_BINARY))
}

fn find_home_from_env(
    home_env: Option<(&str, &str)>,
    default_home_dir: &str,
) -> std::io::Result<AbsolutePathBuf> {
    // Honor an explicit environment override so callers and tests can select a
    // concrete state directory.
    match home_env {
        Some((env_name, val)) => {
            let path = PathBuf::from(val);
            let metadata = std::fs::metadata(&path).map_err(|err| match err.kind() {
                std::io::ErrorKind::NotFound => std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("{env_name} points to {val:?}, but that path does not exist"),
                ),
                _ => std::io::Error::new(
                    err.kind(),
                    format!("failed to read {env_name} {val:?}: {err}"),
                ),
            })?;

            if !metadata.is_dir() {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("{env_name} points to {val:?}, but that path is not a directory"),
                ))
            } else {
                let canonical = path.canonicalize().map_err(|err| {
                    std::io::Error::new(
                        err.kind(),
                        format!("failed to canonicalize {env_name} {val:?}: {err}"),
                    )
                })?;
                AbsolutePathBuf::from_absolute_path(canonical)
            }
        }
        None => {
            let mut p = home_dir().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not find home directory",
                )
            })?;
            p.push(default_home_dir);
            AbsolutePathBuf::from_absolute_path(p)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CODEX_HOME_DIR;
    use super::CODEX_HOME_ENV;
    use super::UNIVERS_CODE_HOME_DIR;
    use super::find_home_from_env;
    use super::is_univers_code_executable;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use dirs::home_dir;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    #[test]
    fn find_codex_home_env_missing_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let missing = temp_home.path().join("missing-codex-home");
        let missing_str = missing
            .to_str()
            .expect("missing codex home path should be valid utf-8");

        let err = find_home_from_env(Some((CODEX_HOME_ENV, missing_str)), CODEX_HOME_DIR)
            .expect_err("missing CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(
            err.to_string().contains("CODEX_HOME"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_file_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let file_path = temp_home.path().join("codex-home.txt");
        fs::write(&file_path, "not a directory").expect("write temp file");
        let file_str = file_path
            .to_str()
            .expect("file codex home path should be valid utf-8");

        let err = find_home_from_env(Some((CODEX_HOME_ENV, file_str)), CODEX_HOME_DIR)
            .expect_err("file CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("not a directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_valid_directory_canonicalizes() {
        let temp_home = TempDir::new().expect("temp home");
        let temp_str = temp_home
            .path()
            .to_str()
            .expect("temp codex home path should be valid utf-8");

        let resolved = find_home_from_env(Some((CODEX_HOME_ENV, temp_str)), CODEX_HOME_DIR)
            .expect("valid CODEX_HOME");
        let expected = temp_home
            .path()
            .canonicalize()
            .expect("canonicalize temp home");
        let expected = AbsolutePathBuf::from_absolute_path(expected).expect("absolute home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_codex_home_without_env_uses_default_home_dir() {
        let resolved =
            find_home_from_env(/*home_env*/ None, CODEX_HOME_DIR).expect("default CODEX_HOME");
        let mut expected = home_dir().expect("home dir");
        expected.push(".codex");
        let expected = AbsolutePathBuf::from_absolute_path(expected).expect("absolute home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn univers_code_uses_an_isolated_default_home_dir() {
        let resolved = find_home_from_env(/*home_env*/ None, UNIVERS_CODE_HOME_DIR)
            .expect("default UNIVERS_CODE_HOME");
        let mut expected = home_dir().expect("home dir");
        expected.push(UNIVERS_CODE_HOME_DIR);
        let expected = AbsolutePathBuf::from_absolute_path(expected).expect("absolute home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn recognizes_univers_code_executable_on_supported_platforms() {
        #[cfg(unix)]
        assert!(is_univers_code_executable(std::path::Path::new(
            "/usr/local/bin/univers-code"
        )));
        #[cfg(windows)]
        assert!(is_univers_code_executable(std::path::Path::new(
            r"C:\Program Files\Univers Code\univers-code.exe"
        )));
        #[cfg(unix)]
        assert!(!is_univers_code_executable(std::path::Path::new(
            "/usr/local/bin/codex"
        )));
        #[cfg(windows)]
        assert!(!is_univers_code_executable(std::path::Path::new(
            r"C:\Program Files\Codex\codex.exe"
        )));
    }
}

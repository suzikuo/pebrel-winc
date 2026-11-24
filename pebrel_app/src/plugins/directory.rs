//! Declarative directory actions invoked by a host workspace, with no Luau or shell dispatch.

#[cfg(all(windows, feature = "gpui-shell"))]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Error, Result, package::identifier};

const DISTRIBUTION: &str = "{distribution}";
const DIRECTORY: &str = "{directory}";
const MAX_ARGS: usize = 16;
const WINDOWS_EXPLORER: &str = "windows-explorer";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LauncherIcon {
    #[default]
    ExternalLink,
    FolderOpen,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Launcher {
    pub(crate) id: String,
    pub(crate) title: String,
    // Omitting the icon preserves the serialized approval for existing installs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) icon: Option<LauncherIcon>,
    pub(crate) executable: String,
    #[serde(default)]
    pub(crate) windows_args: Vec<String>,
    pub(crate) wsl_args: Option<Vec<String>>,
}

impl Launcher {
    pub(crate) fn opens_explorer(&self) -> bool {
        self.executable == WINDOWS_EXPLORER
    }

    pub(super) fn validate(&self) -> Result<()> {
        if !identifier(&self.id) || !super::package::label(&self.title, 256) {
            return Err(invalid("invalid launcher ID or title"));
        }
        if self.opens_explorer() {
            return if self.windows_args.is_empty() && self.wsl_args.is_none() {
                Ok(())
            } else {
                Err(invalid("the Explorer action accepts only the focused directory"))
            };
        }
        // Validate the Windows syntax on every build, including offline package checks.
        let bytes = self.executable.as_bytes();
        if !matches!(bytes, [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic())
            || !self.executable.to_ascii_lowercase().ends_with(".exe")
            || self.executable.len() > 1024
            || self.executable.chars().any(char::is_control)
            || self.executable[3..].contains([':', '"', '<', '>', '|', '*', '?'])
        {
            return Err(invalid("executable must be an absolute Windows .exe path"));
        }
        validate_arguments(&self.windows_args, false)?;
        if let Some(arguments) = &self.wsl_args {
            validate_arguments(arguments, true)?;
            for placeholder in [DISTRIBUTION, DIRECTORY] {
                if arguments.iter().filter(|arg| arg.as_str() == placeholder).count() != 1 {
                    return Err(invalid("WSL arguments must include each placeholder once"));
                }
            }
        }
        Ok(())
    }
}

fn validate_arguments(arguments: &[String], wsl: bool) -> Result<()> {
    if arguments.len() > MAX_ARGS {
        return Err(invalid("a launcher accepts at most 16 arguments per target"));
    }
    if arguments.iter().any(|arg| {
        if wsl && matches!(arg.as_str(), DISTRIBUTION | DIRECTORY) {
            return false;
        }
        arg.len() > 1024 || arg.contains(['{', '}']) || arg.chars().any(char::is_control)
    }) {
        return Err(invalid("invalid launcher argument or placeholder"));
    }
    Ok(())
}

fn invalid(message: &str) -> Error {
    Error::new("invalid_directory_launcher", message)
}

#[cfg(all(windows, feature = "gpui-shell"))]
#[derive(Clone)]
pub(crate) enum Target {
    Windows(PathBuf),
    Wsl { distribution: String, directory: String },
}

#[cfg(all(windows, feature = "gpui-shell"))]
impl Target {
    pub(crate) fn supports(&self, launcher: &Launcher) -> bool {
        match self {
            Self::Windows(_) => true,
            Self::Wsl { .. } => launcher.opens_explorer() || launcher.wsl_args.is_some(),
        }
    }
}

#[cfg(all(windows, feature = "gpui-shell"))]
impl Launcher {
    pub(super) fn spawn(&self, target: Target) -> Result<()> {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};

        if self.opens_explorer() {
            return open_explorer(target);
        }
        let executable = PathBuf::from(&self.executable);
        let mut command = Command::new(&executable);
        match target {
            Target::Windows(directory) if directory.is_absolute() => {
                command.current_dir(directory).args(&self.windows_args);
            },
            Target::Wsl { distribution, directory } if directory.starts_with('/') => {
                let arguments = self.wsl_args.as_ref().ok_or_else(|| {
                    Error::new("unsupported_directory_target", "this launcher does not support WSL")
                })?;
                let arguments = arguments.iter().map(|arg| match arg.as_str() {
                    DISTRIBUTION => distribution.as_str(),
                    DIRECTORY => directory.as_str(),
                    _ => arg.as_str(),
                });
                command
                    .current_dir(executable.parent().expect("absolute executable"))
                    .args(arguments);
            },
            _ => return Err(Error::new("unsupported_directory_target", "no absolute directory")),
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW for console launchers
            .spawn()
            .map_err(|error| Error::new("directory_launch_failed", error))?;
        Ok(())
    }
}

#[cfg(all(windows, feature = "gpui-shell"))]
fn open_explorer(target: Target) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{PCWSTR, w};

    let directory = match target {
        Target::Windows(directory) if directory.is_absolute() => directory,
        Target::Wsl { distribution, directory } if directory.starts_with('/') => {
            if !explorer_component(&distribution)
                || !directory.split('/').filter(|part| !part.is_empty()).all(explorer_component)
            {
                return Err(Error::new("unsupported_directory_target", "invalid WSL folder"));
            }
            let unc = crate::shell_detect::wsl_unc_path(&distribution, &directory);
            if unc.is_dir() {
                unc
            } else {
                PathBuf::from(format!(r"\\wsl$\{distribution}{}", directory.replace('/', "\\")))
            }
        },
        _ => return Err(Error::new("unsupported_directory_target", "no absolute directory")),
    };
    if !directory.is_dir() {
        return Err(Error::new("directory_launch_failed", "folder is unavailable"));
    }
    let wide: Vec<u16> = directory.as_os_str().encode_wide().chain([0]).collect();
    // A folder is the ShellExecute target. No arguments from the plugin or the pane.
    let result = unsafe {
        ShellExecuteW(None, w!("explore"), PCWSTR(wide.as_ptr()), None, None, SW_SHOWNORMAL)
    };
    if result.0 as isize <= 32 {
        return Err(Error::new(
            "directory_launch_failed",
            format!("Explorer error {}", result.0 as isize),
        ));
    }
    Ok(())
}

#[cfg(any(all(windows, feature = "gpui-shell"), test))]
fn explorer_component(part: &str) -> bool {
    !part.is_empty()
        && !part.ends_with(['.', ' '])
        && !part.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|'])
        && !part.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_rejects_arguments_and_unrepresentable_guest_components() {
        let mut launcher = Launcher {
            id: "folder".into(),
            title: "Explorer".into(),
            icon: None,
            executable: WINDOWS_EXPLORER.into(),
            windows_args: Vec::new(),
            wsl_args: None,
        };
        assert!(launcher.validate().is_ok());
        launcher.windows_args.push("/select".into());
        assert!(launcher.validate().is_err());
        assert!(explorer_component("项目 with spaces"));
        for part in ["..", "trailing.", "trailing ", "C:", "dir\\sub", "name\0"] {
            assert!(!explorer_component(part));
        }
    }
}

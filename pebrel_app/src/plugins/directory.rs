//! Declarative directory actions invoked by a host workspace, with no Luau or shell dispatch.

#[cfg(all(windows, feature = "gpui-shell"))]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Error, Result, package::identifier};

const DISTRIBUTION: &str = "{distribution}";
const DIRECTORY: &str = "{directory}";
const MAX_ARGS: usize = 16;

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
    pub(super) fn validate(&self) -> Result<()> {
        if !identifier(&self.id) || !super::package::label(&self.title, 256) {
            return Err(invalid("invalid launcher ID or title"));
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
            Self::Wsl { .. } => launcher.wsl_args.is_some(),
        }
    }
}

#[cfg(all(windows, feature = "gpui-shell"))]
impl Launcher {
    pub(super) fn spawn(&self, target: Target) -> Result<()> {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};

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

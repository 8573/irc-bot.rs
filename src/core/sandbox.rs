//! Enhance the bot's security with sandboxing.
//!
//! On platforms that Rust considers to be `linux`, `macos`, or `android`, this module uses Servo's
//! `gaol` library to provide sandboxing.
//!
//! On other platforms, no sandboxing is provided.

use super::ErrorKind;
use super::Result;
use std::path::PathBuf;

pub(super) const PLATFORM_HAS_GAOL: bool = cfg!(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
));

pub(crate) const PLATFORM_HAS_SANDBOX: bool = PLATFORM_HAS_GAOL || false;

/// Create a sandbox around the running process that allows the process to initiate network
/// connections and read from the designated data directory, on platforms on which sandboxing is
/// supported by this bot framework.
///
/// Note that the data directory is not expected to contain the bot's primary configuration file,
/// so that file must be read before this function is called.
pub(super) fn activate<P1>(data_dir_path: Option<P1>) -> Result<()>
where
    P1: Into<PathBuf>,
{
    #[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
    return gaol_activate(data_dir_path.map(Into::into));

    #[allow(unreachable_code)]
    {
        debug_assert!(!PLATFORM_HAS_SANDBOX);
        Err(ErrorKind::SandboxUnsupported.into())
    }
}

#[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
pub(super) fn gaol_activate(data_dir_path: Option<PathBuf>) -> Result<()> {
    use gaol::profile::AddressPattern;
    use gaol::profile::Operation;
    use gaol::profile::PathPattern;
    use gaol::profile::Profile;
    use gaol::sandbox::ChildSandbox;
    use gaol::sandbox::ChildSandboxMethods;

    let mut allowed_operations = Vec::new();

    allowed_operations.push(Operation::NetworkOutbound(AddressPattern::All));

    if let Some(path) = data_dir_path {
        allowed_operations.push(Operation::FileReadAll(PathPattern::Subpath(path)));
    }

    Profile::new(allowed_operations)
        .and_then(|profile| ChildSandbox::new(profile).activate())
        .map_err(|()| ErrorKind::SandboxFailed.into())
}

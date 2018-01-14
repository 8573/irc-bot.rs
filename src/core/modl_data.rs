use super::Result;
use std::borrow::Cow;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::io::Read;
use std::panic::RefUnwindSafe;
use std::panic::UnwindSafe;
use std::path::Path;
use std::path::PathBuf;

pub trait ModuleDataProvider: Send + Sync + UnwindSafe + RefUnwindSafe + 'static {
    fn read_str(&self, path: &Path) -> Result<Cow<str>>;

    fn read_bytes(&self, path: &Path) -> Result<Cow<[u8]>>;

    fn fs_path(&self) -> Option<&Path>;
}

pub struct ModuleDataDir {
    path: PathBuf,
}

impl ModuleDataProvider for ModuleDataDir {
    fn read_str(&self, path: &Path) -> Result<Cow<str>> {
        let mut data = Default::default();
        File::open(self.path.join(path))?.read_to_string(&mut data)?;
        Ok(data.into())
    }

    fn read_bytes(&self, path: &Path) -> Result<Cow<[u8]>> {
        let mut data = Default::default();
        File::open(self.path.join(path))?.read_to_end(&mut data)?;
        Ok(data.into())
    }

    fn fs_path(&self) -> Option<&Path> {
        Some(&self.path)
    }
}

impl<'a> From<&'a Path> for ModuleDataDir {
    fn from(path: &Path) -> Self {
        ModuleDataDir { path: path.to_owned().into() }
    }
}

impl From<Box<Path>> for ModuleDataDir {
    fn from(path: Box<Path>) -> Self {
        ModuleDataDir { path: path.into() }
    }
}

impl From<PathBuf> for ModuleDataDir {
    fn from(path: PathBuf) -> Self {
        ModuleDataDir { path: path.into() }
    }
}

impl<'a> From<&'a OsStr> for ModuleDataDir {
    fn from(path: &OsStr) -> Self {
        ModuleDataDir { path: path.to_owned().into() }
    }
}

impl From<OsString> for ModuleDataDir {
    fn from(path: OsString) -> Self {
        ModuleDataDir { path: path.into() }
    }
}

impl<'a> From<&'a str> for ModuleDataDir {
    fn from(path: &str) -> Self {
        ModuleDataDir { path: path.to_owned().into() }
    }
}

impl From<String> for ModuleDataDir {
    fn from(path: String) -> Self {
        ModuleDataDir { path: path.into() }
    }
}

/// A `ModuleDataProvider` that always returns "file not found" errors.
pub struct NullModuleDataProvider;

impl ModuleDataProvider for NullModuleDataProvider {
    fn read_str(&self, _path: &Path) -> Result<Cow<str>> {
        Err(io::Error::from(io::ErrorKind::NotFound).into())
    }

    fn read_bytes(&self, _path: &Path) -> Result<Cow<[u8]>> {
        Err(io::Error::from(io::ErrorKind::NotFound).into())
    }

    fn fs_path(&self) -> Option<&Path> {
        None
    }
}

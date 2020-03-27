use std::io;
use std::path;

use git2;
use clap::{ArgMatches};
use err_derive::Error;

use crate::gpm::package::Package;

pub mod install;
pub mod download;
pub mod update;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error(display = "IO error")]
    IOError(#[error(source)] io::Error),
    #[error(display = "git error")]
    GitError(#[error(source)] git2::Error),
    #[error(display = "no matching version for package {}", package)]
    NoMatchingVersionError { package: Package },
    #[error(display = "the path {:?} (passed via --prefix) does not exist, use --force to create it", prefix)]
    PrefixNotFoundError { prefix: path::PathBuf },
    #[error(display = "the path {:?} (passed via --prefix) is not a directory", prefix)]
    PrefixIsNotDirectoryError { prefix: path::PathBuf },
    #[error(display = "package {} was not successfully installed, check the logs for warnings/errors", package)]
    PackageNotInstalledError { package: Package },
}

type CommandResult = std::result::Result<bool, CommandError>;

pub trait Command {

    fn matched_args<'a, 'b>(&self, args : &'a ArgMatches<'b>) -> Option<&'a ArgMatches<'b>>;
    fn run(&self, args: &ArgMatches) -> CommandResult;
}

pub fn commands() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(install::InstallPackageCommand {}),
        Box::new(download::DownloadPackageCommand {}),
        Box::new(update::UpdatePackageRepositoriesCommand {}),
    ]
}

use std::path;
use std::fs;

use console::style;
use tempfile::tempdir;
use url::{Url};
use indicatif::{ProgressBar, ProgressStyle};
use clap::{ArgMatches};

use gitlfs::lfs;

use crate::gpm;
use crate::gpm::command::{Command, CommandError, CommandResult};

pub struct CleanCacheCommand {
}

impl CleanCacheCommand {
    fn run_clean(&self) -> Result<bool, CommandError> {
        info!("running the \"clean\" command");

        let cache = gpm::file::get_or_init_cache_dir().map_err(CommandError::IOError)?;

        if !cache.exists() || !cache.is_dir() {
            warn!("{} does not exist or is not a directory", cache.display());

            return Ok(false);
        }

        debug!("removing {}", cache.display());
        fs::remove_dir_all(&cache).map_err(CommandError::IOError)?;
        debug!("{} removed", cache.display());

        Ok(true)
    }
}

impl Command for CleanCacheCommand {
    fn matched_args<'a, 'b>(&self, args : &'a ArgMatches<'b>) -> Option<&'a ArgMatches<'b>> {
        args.subcommand_matches("clean")
    }

    fn run(&self, _args: &ArgMatches) -> CommandResult {
        match self.run_clean() {
            Ok(success) => {
                if success {
                    info!("cache successfully cleaned");
                    Ok(true)
                } else {
                    error!("cache has not been cleaned, check the logs for warnings/errors");
                    Ok(false)
                }
            },
            Err(e) => Err(e),
        }
    }
}

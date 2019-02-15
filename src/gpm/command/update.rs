use std::io;
use std::io::prelude::*;
use std::fs;

use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use clap::{ArgMatches};

use crate::gpm;
use crate::gpm::command::{Command, CommandError};

pub struct UpdatePackageRepositoriesCommand {
}

impl UpdatePackageRepositoriesCommand {
    fn run_update(&self) -> Result<bool, CommandError> {
        info!("running the \"update\" command");

        println!(
            "{} all repositories",
            gpm::style::command(&String::from("Updating")),
        );

        let dot_gpm_dir = gpm::file::get_or_init_dot_gpm_dir().map_err(CommandError::IO)?;
        let source_file_path = dot_gpm_dir.to_owned().join("sources.list");

        if !source_file_path.exists() || !source_file_path.is_file() {
            warn!("{} does not exist or is not a file", source_file_path.display());

            return Ok(false);
        }

        let file = fs::File::open(source_file_path)?;
        let mut num_repos = 0;
        let mut num_updated = 0;
        let mut repos : Vec<String> = Vec::new();

        for line in io::BufReader::new(file).lines() {
            let line = String::from(line.unwrap().trim());

            if line == "" {
                continue;
            }

            num_repos += 1;

            repos.push(line);
        }

        let pb = ProgressBar::new(repos.len() as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:30.cyan/blue}] {pos}/{len} {wide_msg}")
            .progress_chars("#>-"));
        for remote in repos {
            info!("updating repository {}", remote);

            pb.set_message(&format!("updating {}", &remote));

            match gpm::git::get_or_clone_repo(&remote) {
                Ok((repo, _is_new_repo)) => {
                    match gpm::git::pull_repo(&repo) {
                        Ok(()) => {
                            pb.inc(1);
                            num_updated += 1;
                            info!("updated repository {}", remote);
                        },
                        Err(e) => {
                            warn!("could not update repository: {}", e);
                        }
                    }
                },
                Err(e) => {
                    warn!("could not initialize repository: {}", e);
                }
            }
        }

        pb.finish_with_message("updated repositories");

        if num_updated > 1 {
            info!("updated {}/{} repositories", num_updated, num_repos);
        } else {
            info!("updated {}/{} repository", num_updated, num_repos);
        }

        let success = num_updated == num_repos;

        if success {
            println!("{}", style("Done!").green());
        }

        Ok(success)
    }
}

impl Command for UpdatePackageRepositoriesCommand {
    fn matched_args<'a, 'b>(&self, args : &'a ArgMatches<'b>) -> Option<&'a ArgMatches<'b>> {
        args.subcommand_matches("update")
    }

    fn run(&self, _args: &ArgMatches) -> Result<bool, CommandError> {
        match self.run_update() {
            Ok(success) => {
                if success {
                    info!("package repositories successfully updated");
                    Ok(true)
                } else {
                    error!("package repositories have not been updated, check the logs for warnings/errors");
                    Ok(false)
                }
            },
            Err(e) => Err(e),
        }
    }
}

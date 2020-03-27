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
use crate::gpm::package::Package;

pub struct InstallPackageCommand {
}

impl InstallPackageCommand {
    fn run_install(
        &self,
        package : &Package,
        prefix : &path::Path,
        force : bool,
    ) -> Result<bool, CommandError> {
        info!("running the \"install\" command for package {} at revision {}", package.name(), package.version());

        println!(
            "{} package {}",
            gpm::style::command(&String::from("Installing")),
            &package,
        );

        println!(
            "{} Resolving package",
            style("[1/3]").bold().dim(),
        );

        let (repo, refspec) = gpm::git::find_or_init_repo(&package)?;
        let remote = repo.find_remote("origin")?.url().unwrap().to_owned();

        info!("revision {:?} found as refspec {} in repository {}", package.version(), &refspec, remote);

        let oid = repo.refname_to_id(&refspec).map_err(CommandError::GitError)?;
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();

        debug!("move repository HEAD to {}", &refspec);
        repo.set_head_detached(oid).map_err(CommandError::GitError)?;
        repo.checkout_head(Some(&mut builder)).map_err(CommandError::GitError)?;

        let workdir = repo.workdir().unwrap();
        let package_filename = format!("{}.tar.gz", package.name());
        let package_path = workdir.join(package.name()).join(&package_filename);
        let parsed_lfs_link_data = lfs::parse_lfs_link_file(&package_path);

        let (total, extracted) = if parsed_lfs_link_data.is_ok() {
            let (_oid, size) = parsed_lfs_link_data.unwrap().unwrap();
            let size = size.parse::<usize>().unwrap();

            println!("{} Downloading package", style("[2/3]").bold().dim());

            info!("start downloading archive {} from LFS", package_filename);

            let tmp_dir = tempdir().map_err(CommandError::IOError)?;
            let tmp_package_path = tmp_dir.path().to_owned().join(&package_filename);
            let remote_url : Url = remote.parse().unwrap();
            // If we have a username/password in the remote URL, we assume we can use
            // HTTP basic auth and we don't even try to find SSH credentials.
            let (key, passphrase) = if remote_url.username() != "" && remote_url.password().is_some() {
                (None, None)
            } else {
                gpm::ssh::get_ssh_key_and_passphrase(&String::from(remote_url.host_str().unwrap()))
            };
            let file = fs::OpenOptions::new()
                .write(true)
                .read(true)
                .create(true)
                .truncate(true)
                .open(&tmp_package_path)
                .expect("unable to open LFS object target file");
            let pb = ProgressBar::new(size as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .progress_chars("#>-"));
            pb.enable_steady_tick(200);
            let mut progress = gpm::file::FileProgressWriter::new(
                file,
                size,
                |p : usize, _t : usize| {
                    pb.set_position(p as u64);
                }
            );
            
            lfs::resolve_lfs_link(
                remote.parse().unwrap(),
                Some(refspec.clone()),
                &package_path,
                &mut progress,
                key,
                passphrase,
            ).map_err(CommandError::IOError)?;

            pb.finish();
            
            println!(
                "{} Extracting package in {:?}",
                style("[3/3]").bold().dim(),
                prefix,
            );

            gpm::file::extract_package(&tmp_package_path, &prefix, force).map_err(CommandError::IOError)?
        } else {
            warn!("package {} does not use LFS", package.name());

            println!(
                "{} Extracting package in {:?}",
                style("[3/3]").bold().dim(),
                prefix,
            );

            gpm::file::extract_package(&package_path, &prefix, force).map_err(CommandError::IOError)?
        };

        if total == 0 {
            warn!("no files to extract from the archive {}: is your package archive empty?", package_filename);
        }

        // ? FIXME: reset back to HEAD?

        if extracted != 0 {
            println!("{}", style("Done!").green());
        }

        Ok(extracted != 0)
    }
}

impl Command for InstallPackageCommand {
    fn matched_args<'a, 'b>(&self, args : &'a ArgMatches<'b>) -> Option<&'a ArgMatches<'b>> {
        args.subcommand_matches("install")
    }

    fn run(&self, args: &ArgMatches) -> CommandResult {
        let force = args.is_present("force");
        let prefix = path::Path::new(args.value_of("prefix").unwrap());

        if !prefix.exists() && !force {
            Err(CommandError::PrefixNotFoundError { prefix: prefix.to_path_buf() })
        } else if prefix.exists() && !prefix.is_dir() {
            Err(CommandError::PrefixIsNotDirectoryError { prefix: prefix.to_path_buf() })
        } else {
            let package = Package::parse(&String::from(args.value_of("package").unwrap()));

            debug!("parsed package: {:?}", &package);

            match self.run_install(&package, &prefix, force) {
                Ok(success) => if success {
                    info!("package {} successfully installed in {}", package.name(), prefix.display());
                    Ok(success)
                } else {
                    Err(CommandError::PackageNotInstalledError { package })
                },
                Err(e) => {
                    Err(e)
                },
            }
        }
    }
}

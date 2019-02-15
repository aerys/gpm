use std::fs;
use std::env;

use console::style;
use url::{Url};
use indicatif::{ProgressBar, ProgressStyle};
use clap::{ArgMatches};

use gitlfs::lfs;

use crate::gpm;
use crate::gpm::command::{Command, CommandError};

pub struct DownloadPackageCommand {
}

impl DownloadPackageCommand {
    fn run_download(
        &self,
        remote : Option<String>,
        package : &String,
        revision : &String,
        force : bool,
    ) -> Result<bool, CommandError> {
        info!("running the \"download\" command for package {} at revision {}", package, revision);

        println!(
            "{} package {} at revision {}",
            gpm::style::command(&String::from("Downloading")),
            gpm::style::package_name(&package),
            gpm::style::revision(&revision),
        );

        println!(
            "{} Resolving package",
            style("[1/2]").bold().dim(),
        );

        let (repo, refspec) = match gpm::git::find_or_init_repo(remote, package, revision)? {
            Some(repo) => repo,
            None => panic!("package/revision was not found in any repository"),
        };

        let remote = repo.find_remote("origin")?.url().unwrap().to_owned();

        info!("revision {} found as refspec {} in repository {}", &revision, &refspec, remote);

        let oid = repo.refname_to_id(&refspec).map_err(CommandError::Git)?;
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();

        debug!("move repository HEAD to {}", revision);
        repo.set_head_detached(oid).map_err(CommandError::Git)?;
        repo.checkout_head(Some(&mut builder)).map_err(CommandError::Git)?;

        let workdir = repo.workdir().unwrap();
        let package_filename = format!("{}.tar.gz", package);
        let package_path = workdir.join(package).join(&package_filename);
        let cwd_package_path = env::current_dir().unwrap().join(&package_filename);

        if cwd_package_path.exists() && !force {
            error!("path {} already exist, use --force to override", cwd_package_path.display());
            return Ok(false);
        }

        let parsed_lfs_link_data = lfs::parse_lfs_link_file(&package_path);

        if parsed_lfs_link_data.is_ok() {
            let (_oid, size) = parsed_lfs_link_data.unwrap().unwrap();
            let size = size.parse::<usize>().unwrap();
        
            info!("start downloading archive {} from LFS", package_filename);

            println!(
                "{} Downloading package",
                style("[2/2]").bold().dim(),
            );

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
                .create(true)
                .truncate(true)
                .open(&cwd_package_path)
                .expect("unable to open LFS object target file");
            let pb = ProgressBar::new(size as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({eta}) {wide_msg}")
                .progress_chars("#>-"));
            pb.enable_steady_tick(200); 
            pb.set_message(&format!("downloading {}={}", &package, &refspec));       
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
            ).map_err(CommandError::IO)?;

            pb.finish_with_message(&format!("downloaded {}={}", &package, &refspec));
        } else {
            fs::copy(package_path, cwd_package_path).map_err(CommandError::IO)?;
        }

        // ? FIXME: reset back to HEAD?

        println!("{}", style("Done!").green());

        Ok(true)
    }
}

impl Command for DownloadPackageCommand {
    fn matched_args<'a, 'b>(&self, args : &'a ArgMatches<'b>) -> Option<&'a ArgMatches<'b>> {
        args.subcommand_matches("download")
    }

    fn run(&self, args: &ArgMatches) -> Result<bool, CommandError> {
        let force = args.is_present("force");
        let package = String::from(args.value_of("package").unwrap());
        let (repo, package, revision) = gpm::package::parse_ref(&package);

        if repo.is_some() {
            debug!("parsed package URI: repo = {}, name = {}, revision = {}", repo.to_owned().unwrap(), package, revision);
        } else {
            debug!("parsed package: name = {}, revision = {}", package, revision);
        }

        match self.run_download(repo, &package, &revision, force) {
            Ok(success) => {
                if success {
                    info!("package {} successfully downloaded at revision {}", package, revision);

                    Ok(true)
                } else {
                    error!("package {} has not been downloaded, check the logs for warnings/errors", package);

                    Ok(false)
                }
            },
            Err(e) => Err(e)
        }
    }
}

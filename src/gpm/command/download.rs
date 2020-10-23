use std::fs;
use std::env;
use std::path;

use console::style;
use url::{Url};
use indicatif::{ProgressBar, ProgressStyle};
use clap::{ArgMatches};

use gitlfs::lfs;

use crate::gpm;
use crate::gpm::command::{Command, CommandError, CommandResult};
use crate::gpm::package::Package;

pub struct DownloadPackageCommand {
}

impl DownloadPackageCommand {
    fn run_download(
        &self,
        package : &Package,
        force : bool,
    ) -> Result<bool, CommandError> {
        info!("running the \"download\" command for package {}", package);

        println!(
            "{} package {}",
            gpm::style::command(&String::from("Downloading")),
            package,
        );

        println!(
            "{} Resolving package",
            style("[1/2]").bold().dim(),
        );

        let (repo, refspec) = gpm::git::find_or_init_repo(package)?;
        let remote = repo.find_remote("origin")?.url().unwrap().to_owned();

        info!("{} found as refspec {} in repository {}", package, &refspec, remote);

        let oid = repo.refname_to_id(&refspec).map_err(CommandError::GitError)?;

        package.print_message(oid, &repo);

        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();

        debug!("move repository HEAD to {}", refspec);
        repo.set_head_detached(oid).map_err(CommandError::GitError)?;
        repo.checkout_head(Some(&mut builder)).map_err(CommandError::GitError)?;

        let package_path = package.get_archive_path(Some(path::PathBuf::from(repo.workdir().unwrap())));
        let cwd_package_path = env::current_dir().unwrap().join(&package.get_archive_filename());

        if cwd_package_path.exists() && !force {
            error!("path {} already exist, use --force to override", cwd_package_path.display());
            return Ok(false);
        }

        let parsed_lfs_link_data = lfs::parse_lfs_link_file(&package_path);

        if parsed_lfs_link_data.is_ok() {
            let (oid, size) = parsed_lfs_link_data.unwrap().unwrap();
            let size = size.parse::<usize>().unwrap();
        
            info!("start downloading archive {:?} from LFS", cwd_package_path);

            println!(
                "{} Downloading package",
                style("[2/2]").bold().dim(),
            );

            let file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&cwd_package_path)?;
            let pb = ProgressBar::new(size as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("  [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .progress_chars("#>-"));

            lfs::resolve_lfs_link(
                remote.parse().unwrap(),
                Some(refspec.clone()),
                &package_path,
                &mut pb.wrap_write(file),
                &|repository: Url| {
                    let (k, p) = gpm::ssh::get_ssh_key_and_passphrase(
                        &String::from(repository.host_str().unwrap())
                    );

                    (k.unwrap(), p)
                },
                Some(format!("gpm/{}", env!("VERGEN_SEMVER"))),
            ).map_err(CommandError::GitLFSError)?;

            let mut file = fs::OpenOptions::new()
                .read(true)
                .open(&cwd_package_path)?;
            let archive_oid = lfs::get_oid(&mut file);
            if archive_oid != oid {
                return Err(CommandError::InvalidLFSObjectSignature {
                    expected: oid,
                    got: archive_oid,
                })
            }

            pb.finish();
        } else {
            fs::copy(package_path, cwd_package_path).map_err(CommandError::IOError)?;
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

    fn run(&self, args: &ArgMatches) -> CommandResult {
        let force = args.is_present("force");
        let package = Package::parse(&String::from(args.value_of("package").unwrap()));

        debug!("parsed package: {:?}", &package);

        match self.run_download(&package, force) {
            Ok(success) => {
                if success {
                    info!("package {} successfully downloaded", &package);

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

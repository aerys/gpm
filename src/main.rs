use std::io;
use std::io::prelude::*;
use std::path;
use std::env;
use std::fs;

#[macro_use]
extern crate clap; 
use clap::{App, Arg};

#[macro_use]
extern crate log;

extern crate pretty_env_logger;

extern crate git2;

extern crate gitlfs;
use gitlfs::lfs;

extern crate url;
use url::{Url};

extern crate indicatif;
use indicatif::{ProgressBar, ProgressStyle};

extern crate pest;
#[macro_use]
extern crate pest_derive;

extern crate tempfile;
use tempfile::tempdir;

mod gpm;
use gpm::error::CommandError;

extern crate console;
use console::style;

extern crate regex;

fn clean_command() -> Result<bool, CommandError> {
    info!("running the \"clean\" command");

    let cache = gpm::file::get_or_init_cache_dir().map_err(CommandError::IO)?;

    if !cache.exists() || !cache.is_dir() {
        warn!("{} does not exist or is not a directory", cache.display());

        return Ok(false);
    }

    debug!("removing {}", cache.display());
    fs::remove_dir_all(&cache).map_err(CommandError::IO)?;
    debug!("{} removed", cache.display());

    Ok(true)
}

fn update_command() -> Result<bool, CommandError> {
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

fn download_command(
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
            .template("{spinner:.green} [{elapsed_precise}] [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({eta}){wide_msg}")
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

fn install_command(
    remote : Option<String>,
    package : &String,
    revision : &String,
    prefix : &path::Path,
    force : bool,
) -> Result<bool, CommandError> {
    info!("running the \"install\" command for package {} at revision {}", package, revision);

    println!(
        "{} package {} at revision {}",
        gpm::style::command(&String::from("Installing")),
        gpm::style::package_name(&package),
        gpm::style::revision(&revision),
    );

    println!(
        "{} Resolving package",
        style("[1/3]").bold().dim(),
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

    debug!("move repository HEAD to {}", &refspec);
    repo.set_head_detached(oid).map_err(CommandError::Git)?;
    repo.checkout_head(Some(&mut builder)).map_err(CommandError::Git)?;

    let workdir = repo.workdir().unwrap();
    let package_filename = format!("{}.tar.gz", package);
    let package_path = workdir.join(package).join(&package_filename);
    let parsed_lfs_link_data = lfs::parse_lfs_link_file(&package_path);

    let (total, extracted) = if parsed_lfs_link_data.is_ok() {
        let (_oid, size) = parsed_lfs_link_data.unwrap().unwrap();
        let size = size.parse::<usize>().unwrap();

        println!("{} Downloading package", style("[2/3]").bold().dim());

        info!("start downloading archive {} from LFS", package_filename);

        let tmp_dir = tempdir().map_err(CommandError::IO)?;
        let tmp_package_path = tmp_dir.path().to_owned().join(&package_filename);
        let remote_url : Url = remote.parse().unwrap();
        // If we have a username/password in the remote URL, we assume we can use
        // HTTP basic auth and we don't even try to find SSH credentials.
        let (key, passphrase) = if remote_url.username() != "" && remote_url.password().is_some() {
            (None, None)
        } else {
            gpm::ssh::get_ssh_key_and_passphrase(&String::from(remote_url.host_str().unwrap()))
        };
        let mut file = fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .truncate(true)
            .open(&tmp_package_path)
            .expect("unable to open LFS object target file");
        let pb = ProgressBar::new(size as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({eta}) {wide_msg}")
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
        ).map_err(CommandError::IO)?;

        pb.finish_with_message("downloaded");
        
        println!(
            "{} Extracting package in {:?}",
            style("[3/3]").bold().dim(),
            prefix,
        );

        gpm::file::extract_package(&tmp_package_path, &prefix, force).map_err(CommandError::IO)?
    } else {
        warn!("package {} does not use LFS", package);

        println!(
            "{} Extracting package in {:?}",
            style("[3/3]").bold().dim(),
            prefix,
        );

        gpm::file::extract_package(&package_path, &prefix, force).map_err(CommandError::IO)?
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

fn parse_package_ref(package_ref : &String) -> (Option<String>, String, String) {
    let url = package_ref.parse();

    if url.is_ok() {
        let url : Url = url.unwrap();
        let package_and_version = String::from(url.fragment().unwrap());
        let (_, package, version) = parse_package_ref(&package_and_version);
        let mut remote = url.clone();

        remote.set_fragment(None);

        return (
            Some(String::from(remote.as_str())),
            package,
            version,
        );

    } else {
        if package_ref.contains("=") {
            let parts : Vec<&str> = package_ref.split("=").collect();

            return (
                None,
                parts[0].to_string(),
                parts[1].to_string(),
            );
        }

        if package_ref.contains("/") {
            let parts : Vec<&str> = package_ref.split("/").collect();

            return (
                None,
                parts[0].to_string(),
                package_ref.to_owned(),
            );
        }

        (None, package_ref.to_owned(), String::from("refs/heads/master"))
    }
}

fn main() {
    pretty_env_logger::init_custom_env("GPM_LOG");

    let matches = App::new("gpm")
        .about("Git-based package manager.")
        .version(crate_version!())
        .setting(clap::AppSettings::ArgRequiredElseHelp)
        .subcommand(clap::SubCommand::with_name("install")
            .about("Install a package")
            .arg(Arg::with_name("package"))
            .arg(Arg::with_name("prefix")
                .help("The prefix to the package install path")
                .default_value("/")
                .long("--prefix")
                .required(false)
            )
            .arg(Arg::with_name("force")
                .help("Replace existing files")
                .long("--force")
                .takes_value(false)
                .required(false)
            )
        )
        .subcommand(clap::SubCommand::with_name("download")
            .about("Download a package")
            .arg(Arg::with_name("package"))
            .arg(Arg::with_name("force")
                .help("Replace existing files")
                .long("--force")
                .takes_value(false)
                .required(false)
            )
        )
        .subcommand(clap::SubCommand::with_name("update")
            .about("Update all package repositories")
        )
        .subcommand(clap::SubCommand::with_name("clean")
            .about("Clean all repositories from cache")
        )
        .get_matches();

    if let Some(_matches) = matches.subcommand_matches("update") {
        match update_command() {
            Ok(success) => {
                if success {
                    info!("package repositories successfully updated")
                } else {
                    error!("package repositories have not been updated, check the logs for warnings/errors");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                error!("could not update repositories: {}", e);
                std::process::exit(1);
            },
        }
    }

    if let Some(_matches) = matches.subcommand_matches("clean") {
        match clean_command() {
            Ok(success) => {
                if success {
                    info!("cache successfully cleaned")
                } else {
                    error!("cache has not been cleaned, check the logs for warnings/errors");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                error!("could not clean cache: {}", e);
                std::process::exit(1);
            },
        }
    }

    if let Some(matches) = matches.subcommand_matches("install") {
        let force = matches.is_present("force");
        let prefix = path::Path::new(matches.value_of("prefix").unwrap());

        if !prefix.exists() {
            error!("path {} (passed via --prefix) does not exist", prefix.to_str().unwrap());
            std::process::exit(1);
        }
        if !prefix.is_dir() {
            error!("path {} (passed via --prefix) is not a directory", prefix.to_str().unwrap());
            std::process::exit(1);
        }

        let package = String::from(matches.value_of("package").unwrap());
        let (repo, package, revision) = parse_package_ref(&package);

        if repo.is_some() {
            debug!("parsed package URI: repo = {}, name = {}, revision = {}", repo.to_owned().unwrap(), package, revision);
        } else {
            debug!("parsed package: name = {}, revision = {}", package, revision);
        }

        match install_command(repo, &package, &revision, &prefix, force) {
            Ok(success) => if success {
                info!("package {} successfully installed at revision {} in {}", package, revision, prefix.display())
            } else {
                error!("package {} was not successfully installed at revision {} in {}, check the logs for warnings/errors", package, revision, prefix.display());
                std::process::exit(1);
            },
            Err(e) => {
                error!("could not install package \"{}\" at revision {}: {}", package, revision, e);
                std::process::exit(1);
            },
        }
    }

    if let Some(matches) = matches.subcommand_matches("download") {
        let force = matches.is_present("force");
        let package = String::from(matches.value_of("package").unwrap());
        let (repo, package, revision) = parse_package_ref(&package);

        if repo.is_some() {
            debug!("parsed package URI: repo = {}, name = {}, revision = {}", repo.to_owned().unwrap(), package, revision);
        } else {
            debug!("parsed package: name = {}, revision = {}", package, revision);
        }

        match download_command(repo, &package, &revision, force) {
            Ok(success) => {
                if success {
                    info!("package {} successfully downloaded at revision {}", package, revision);
                } else {
                    error!("package {} has not been downloaded, check the logs for warnings/errors", package);
                    std::process::exit(1);
                }
            },
            Err(e) => {
                error!("could not download package \"{}\" at revision {}: {}", package, revision, e);
                std::process::exit(1);
            },
        };
    }

    std::process::exit(0);
}

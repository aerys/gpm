use std::fmt;
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

extern crate tar;
use tar::Archive;

extern crate tempfile;
use tempfile::tempdir;

extern crate flate2;

#[derive(Debug)]
pub enum CommandError {
    IO(io::Error),
    Git(git2::Error),
}

impl From<io::Error> for CommandError {
    fn from(err: io::Error) -> CommandError {
        CommandError::IO(err)
    }
}

impl From<git2::Error> for CommandError {
    fn from(err: git2::Error) -> CommandError {
        CommandError::Git(err)
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CommandError::IO(e) => write!(f, "{}", e),
            CommandError::Git(s) => write!(f, "{}", s),
        }
    }
}

fn clean_command() -> Result<bool, CommandError> {
    info!("running the \"clean\" command");

    let cache = get_or_init_cache_dir().map_err(CommandError::IO)?;

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

    let dot_gpm_dir = get_or_init_dot_gpm_dir().map_err(CommandError::IO)?;
    let source_file_path = dot_gpm_dir.to_owned().join("sources.list");

    if !source_file_path.exists() || !source_file_path.is_file() {
        warn!("{} does not exist or is not a file", source_file_path.display());

        return Ok(false);
    }

    let file = fs::File::open(source_file_path)?;
    let mut num_repos = 0;
    let mut num_updated = 0;

    for line in io::BufReader::new(file).lines() {
        let line = String::from(line.unwrap().trim());

        if line == "" {
            continue;
        }

        num_repos += 1;

        info!("updating repository {}", line);

        match get_or_clone_repo(&line) {
            Ok((repo, _is_new_repo)) => {
                match pull_repo(&repo) {
                    Ok(()) => {
                        num_updated += 1;
                        info!("updated repository {}", line);
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

    if num_updated > 1 {
        info!("updated {}/{} repositories", num_updated, num_repos);
    } else {
        info!("updated {}/{} repository", num_updated, num_repos);
    }

    Ok(num_updated > 0)
}

fn download_command(
    remote : Option<String>,
    package : &String,
    revision : &String,
    force : bool,
) -> Result<bool, CommandError> {
    info!("running the \"download\" command for package {} at revision {}", package, revision);

    let (repo, refspec) = match find_or_init_repo(remote, package, revision)? {
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

    if lfs::parse_lfs_link_file(&package_path).is_ok() {
        info!("start downloading archive {} from LFS", package_filename);
        lfs::resolve_lfs_link(
            remote.parse().unwrap(),
            Some(refspec),
            &package_path,
            Some(&cwd_package_path),
            match env::var("GPM_SSH_KEY") {
                Ok(k) => Some(k),
                Err(_) => None,
            },
            match env::var("GPM_SSH_PASS") {
                Ok(k) => Some(k),
                Err(_) => None,
            },
        ).map_err(CommandError::IO)?;
    } else {
        fs::copy(package_path, cwd_package_path).map_err(CommandError::IO)?;
    }

    // ? FIXME: reset back to HEAD?

    Ok(true)
}
fn find_repo_by_package_and_revision(
    package : &String,
    revision : &String,
) -> Result<Option<(git2::Repository, String)>, CommandError> {
    let dot_gpm_dir = get_or_init_dot_gpm_dir().map_err(CommandError::IO)?;
    let source_file_path = dot_gpm_dir.to_owned().join("sources.list");
    let file = fs::File::open(source_file_path)?;

    for line in io::BufReader::new(file).lines() {
        let line = String::from(line.unwrap().trim());

        debug!("searching for revision {} in repository {}", revision, line);

        let path = remote_url_to_cache_path(&line)?;
        let repo = git2::Repository::open(path).map_err(CommandError::Git)?;

        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();
        repo.set_head("refs/heads/master")?;
        repo.checkout_head(Some(&mut builder))?;

        match revision_to_refspec(&repo, &package, &revision) {
            Some(refspec) => {
                debug!("revision {} found with refspec {}", revision, refspec);

                let mut builder = git2::build::CheckoutBuilder::new();
                builder.force();
                repo.set_head(&refspec)?;
                repo.checkout_head(Some(&mut builder))?;

                if package_archive_is_in_repo(&repo, package) {
                    debug!("package archive {}.tar.gz found in refspec {}", package, &refspec);
                    return Ok(Some((repo, refspec)));
                } else {
                    debug!("package archive {}.tar.gz cound not be found in refspec {}, skipping to next repository", package, &refspec);
                    continue;
                }
            },
            None => {
                debug!("revision not found, skipping to next repository");
                continue;
            }
        };
    }

    Ok(None)
}

fn package_archive_is_in_repo(repo : &git2::Repository, package : &String) -> bool {
    let archive_filename = format!("{}.tar.gz", &package);
    let mut path = repo.workdir().unwrap().to_owned();

    path.push(package);
    path.push(archive_filename);

    return path.exists();
}

fn revision_to_refspec(
    repo : &git2::Repository,
    package : &String,
    revision : &String,
) -> Option<String> {
    if repo.refname_to_id(&revision).is_ok() {
            return Some(revision.to_owned());
    }

    let tag_refspec = format!("refs/tags/{}", &revision);
    if repo.refname_to_id(&tag_refspec).is_ok() {
        return Some(tag_refspec);
    }

    let tag_refspec = format!("refs/tags/{}/{}", &package, &revision);
    if repo.refname_to_id(&tag_refspec).is_ok() {
        return Some(tag_refspec);
    }

    let branch_refspec = format!("refs/heads/{}", &revision);
    if repo.refname_to_id(&branch_refspec).is_ok() {
        return Some(branch_refspec);
    }

    return None;
}

fn find_or_init_repo(
    remote : Option<String>,
    package: &String,
    revision : &String,
) -> Result<Option<(git2::Repository, String)>, CommandError> {

    match remote {
        Some(remote) => {
            let (repo, is_new_repo) = get_or_clone_repo(&remote)?;

            if !is_new_repo {
                pull_repo(&repo).map_err(CommandError::Git)?;
            }

            match revision_to_refspec(&repo, package, revision) {
                Some(refspec) => Ok(Some((repo, refspec))),
                // We could not find the revision in the specified remote.
                // So we make the repo throw an error on purpose:
                None => Err(CommandError::Git(repo.refname_to_id(revision).err().unwrap()))
            }
        },
        None => {
            debug!("no specific remote provided: searching for package {} at revision {}", package, revision);

            find_repo_by_package_and_revision(package, revision)
        },
    }
}

fn install_command(
    remote : Option<String>,
    package : &String,
    revision : &String,
    prefix : &path::Path,
    force : bool,
) -> Result<bool, CommandError> {
    info!("running the \"install\" command for package {} at revision {}", package, revision);

    // ! FIXME: search in all repos if there is no remote provided

    let (repo, refspec) = match find_or_init_repo(remote, package, revision)? {
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

    let (total, extracted) = if lfs::parse_lfs_link_file(&package_path).is_ok() {
        let tmp_dir = tempdir().map_err(CommandError::IO)?;
        let tmp_package_path = tmp_dir.path().to_owned().join(&package_filename);

        info!("start downloading archive {} from LFS", package_filename);
        lfs::resolve_lfs_link(
            remote.parse().unwrap(),
            Some(refspec),
            &package_path,
            Some(&tmp_package_path),
            match env::var("GPM_SSH_KEY") {
                Ok(k) => Some(k),
                Err(_) => None,
            },
            match env::var("GPM_SSH_PASS") {
                Ok(k) => Some(k),
                Err(_) => None,
            },
        ).map_err(CommandError::IO)?;
        
        extract_package(&tmp_package_path, &prefix, force).map_err(CommandError::IO)?
    } else {
        warn!("package {} does not use LFS", package);

        extract_package(&package_path, &prefix, force).map_err(CommandError::IO)?
    };

    if total == 0 {
        warn!("no files to extract from the archive {}: is your package archive empty?", package_filename);
    }

    // ? FIXME: reset back to HEAD?

    Ok(extracted != 0)
}

fn pull_repo(repo : &git2::Repository) -> Result<(), git2::Error> {
    info!("fetching changes for repository {}", repo.workdir().unwrap().display());

    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(git_credentials_callback);

    let mut opts = git2::FetchOptions::new();
    opts.remote_callbacks(callbacks);
    opts.download_tags(git2::AutotagOption::All);

    let mut origin_remote = repo.find_remote("origin")?;
    origin_remote.fetch(&["master"], Some(&mut opts), None)?;

    let oid = repo.refname_to_id("refs/remotes/origin/master")?;
    let object = repo.find_object(oid, None)?;
    repo.reset(&object, git2::ResetType::Hard, None)?;

    let mut builder = git2::build::CheckoutBuilder::new();
    builder.force();
    repo.set_head("refs/heads/master")?;
    repo.checkout_head(Some(&mut builder))?;

    Ok(())
}

fn extract_package(path : &path::Path, prefix : &path::Path, force : bool) -> Result<(u32, u32), io::Error> {
    debug!("attempting to extract package archive {} in {}", path.display(), prefix.display());

    let compressed_file = fs::File::open(&path)?;
    let mut file = tempfile::tempfile().unwrap();

    {
        let mut writer = io::BufWriter::new(&file);
        let reader = io::BufReader::new(&compressed_file);
        let mut decoder = flate2::read::GzDecoder::new(reader);

        debug!("start decoding {} in temporary file", path.display());

        io::copy(&mut decoder, &mut writer).unwrap();

        debug!("{} decoded", path.display());
    }

    debug!("start extracting archive into {}", prefix.display());

    file.seek(io::SeekFrom::Start(0))?;

    let mut num_extracted_files = 0;
    let mut num_files = 0;
    let reader = io::BufReader::new(&file);

    let mut ar = Archive::new(reader);
    for file in ar.entries().unwrap() {
        let mut file = file.unwrap();
        let path = prefix.to_owned().join(file.path().unwrap());

        num_files += 1;

        if path.exists() {
            if !force {
                warn!(
                    "{} not extracted: path already exist, use --force to override",
                    path.display()
                );
                continue;
            }

            debug!("{} already exists and --force in use: removing", &path.display());
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
        }

        file.unpack_in(prefix).unwrap();

        debug!(
            "extracted file {} ({} bytes)",
            path.display(),
            file.header().size().unwrap(),
        );

        num_extracted_files += 1;
    }

    info!("extracted {}/{} file(s)", num_extracted_files, num_files);

    Ok((num_files, num_extracted_files))
}

fn get_or_init_dot_gpm_dir() -> Result<std::path::PathBuf, io::Error> {
    let dot_gpm = std::env::home_dir().unwrap().join(".gpm");

    if !dot_gpm.exists() {
        return match std::fs::create_dir_all(&dot_gpm) {
            Ok(()) => Ok(dot_gpm),
            Err(e) => Err(e)
        }
    }

    Ok(dot_gpm)
}

fn get_or_init_cache_dir() -> Result<std::path::PathBuf, io::Error> {
    let dot_gpm = get_or_init_dot_gpm_dir()?;
    let cache = dot_gpm.join("cache");

    if !cache.exists() {
        return match std::fs::create_dir_all(&cache) {
            Ok(()) => Ok(cache),
            Err(e) => Err(e)
        }
    }

    Ok(cache)
}

pub fn git_credentials_callback(
    _user: &str,
    _user_from_url: Option<&str>,
    _cred: git2::CredentialType,
) -> Result<git2::Cred, git2::Error> {
    let user = _user_from_url.unwrap_or("git");

    if _cred.contains(git2::CredentialType::USERNAME) {
        return git2::Cred::username(user);
    }

    match env::var("GPM_SSH_KEY") {
        Ok(k) => {
            let key_path = path::Path::new(&k);
            let (pass_string, has_pass) = match env::var("GPM_SSH_PASS") {
                Ok(p) => (p, true),
                Err(_) => (String::new(), false),
            };
            let pass_opt = if has_pass {
                Some(pass_string.as_str())
            } else {
                None
            };

            if key_path.exists() {
                debug!("authenticate with user {} and private key located in {}", user, k);

                git2::Cred::ssh_key(user, None, key_path, pass_opt)
            } else {
                debug!("authenticate with user {} and private key stored in the GPM_SSH_KEY env var", user);

                git2::Cred::ssh_key_from_memory(user, None, &k, pass_opt)
            }
        },
        _ => Err(git2::Error::from_str("unable to get private key from GPM_SSH_KEY")),
    }
}

fn remote_url_to_cache_path(remote : &String) -> Result<path::PathBuf, CommandError> {
    let cache = get_or_init_cache_dir().map_err(CommandError::IO)?;
    let data_url = match Url::parse(remote) {
        Ok(data_url) => data_url,
        Err(e) => panic!("failed to parse remote url: {}", e),
    };

    let mut path = path::PathBuf::new();
    path.push(cache);
    path.push(data_url.host_str().unwrap());
    path.push(&data_url.path()[1..]);

    Ok(path)
}

fn get_or_clone_repo(remote : &String) -> Result<(git2::Repository, bool), CommandError> {
    let path = remote_url_to_cache_path(remote)?;

    if path.exists() {
        debug!("use existing repository already in cache {}", path.to_str().unwrap());
        return Ok((git2::Repository::open(path)?, false));
    }

    match path.parent() {
        Some(parent) => if !parent.exists() {
            debug!("create missing parent directory {}", parent.display());
            fs::create_dir_all(parent).map_err(CommandError::IO)?;
        },
        None => ()
    };

    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(git_credentials_callback);

    let mut opts = git2::FetchOptions::new();
    opts.remote_callbacks(callbacks);
    opts.download_tags(git2::AutotagOption::All);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(opts);
    builder.branch("master");

    debug!("start cloning repository {} in {}", remote, path.to_str().unwrap());

    // ! FIXME: check .gitattributes for LFS, warn! if relevant
    
    match builder.clone(remote, &path) {
        Ok(r) => {
            debug!("repository cloned");

            Ok((r, true))
        },
        Err(e) => Err(CommandError::Git(e))
    }
}

fn default_port(url: &Url) -> Result<u16, ()> {
    match url.scheme() {
        "ssh" => Ok(22),
        "git" => Ok(9418),
        "git+ssh" => Ok(22),
        "git+https" => Ok(443),
        "git+http" => Ok(80),
        _ => Err(()),
    }
}

fn parse_package_uri(url_or_refspec : &String) -> Result<(Option<String>, String, String), url::ParseError> {
    let url = url_or_refspec.parse();

    if url.is_ok() {
        let url : Url = url.unwrap();
        let package_and_revision : Vec<&str> = url.fragment().unwrap().split("/").collect();
        let repository = format!(
            "{}://{}{}",
            url.scheme(),
            url.with_default_port(default_port).unwrap(),
            url.path(),
        );

        return Ok((
            Some(repository),
            String::from(package_and_revision[0]),
            String::from(package_and_revision[1])
        ));
    }

    if url_or_refspec.contains("=") {
        let parts : Vec<&str> = url_or_refspec.split("=").collect();

        return Ok((
            None,
            parts[0].to_string(),
            parts[1].to_string(),
        ))
    }

    if url_or_refspec.contains("/") {
        let parts : Vec<&str> = url_or_refspec.split("/").collect();

        return Ok((
            None,
            parts[0].to_string(),
            url_or_refspec.to_owned(),
        ));
    }

    Ok((None, url_or_refspec.to_owned(), String::from("refs/heads/master")))
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
        let (repo, package, revision) = parse_package_uri(&package)
            .expect("unable to parse package URI");

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
        let (repo, package, revision) = parse_package_uri(&package)
            .expect("unable to parse package URI");

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

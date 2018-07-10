use std::fmt;
use std::io;
use std::io::prelude::*;
use std::path;
use std::env;
use std::fs;

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

extern crate zip;

extern crate tempfile;
use tempfile::tempdir;

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
    info!("run clean command");

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
    info!("run update command");

    let dot_gpm_dir = get_or_init_dot_gpm_dir().map_err(CommandError::IO)?;
    let source_file_path = dot_gpm_dir.to_owned().join("sources.list");

    if !source_file_path.exists() || !source_file_path.is_file() {
        warn!("{} does not exist or is not a file", source_file_path.display());

        return Ok(false);
    }

    let file = fs::File::open(source_file_path)?;
    let mut num_repos = 0;

    for line in io::BufReader::new(file).lines() {
        let line = line.unwrap();

        info!("updating repository {}", line);

        let (repo, _is_new_repo) = get_or_init_repo(&line)?;

        pull_repo(&repo).map_err(CommandError::Git)?;

        info!("updated repository {}", line);

        num_repos += 1;
    }

    if num_repos > 1 {
        info!("updated {} repositories", num_repos);
    } else {
        info!("updated {} repository", num_repos);
    }

    Ok(true)
}

fn download_command(
    remote : Option<String>,
    package : &String,
    version : &String,
    force : bool,
) -> Result<bool, CommandError> {
    info!("run download command for package {} at version {}", package, version);

    let repo = match find_or_init_repo(remote, package, version)? {
        Some(repo) => repo,
        None => panic!("package was not found in any repository"),
    };
    let remote = repo.find_remote("origin")?.url().unwrap().to_owned();

    info!("package found in repository {}", remote);

    let refspec = format!("refs/tags/{}/{}", package, version);
    let oid = repo.refname_to_id(&refspec).map_err(CommandError::Git)?;
    let mut builder = git2::build::CheckoutBuilder::new();
    builder.force();

    debug!("move repository HEAD to tag {}/{}", package, version);
    repo.set_head_detached(oid).map_err(CommandError::Git)?;
    repo.checkout_head(Some(&mut builder)).map_err(CommandError::Git)?;

    let workdir = repo.workdir().unwrap();
    let package_filename = format!("{}.zip", package);
    let package_path = workdir.join(package).join(&package_filename);
    let cwd_package_path = env::current_dir().unwrap().join(&package_filename);

    if cwd_package_path.exists() && !force {
        info!("path {} already exist, use --force to override", cwd_package_path.display());
        return Ok(false);
    }

    if lfs::parse_lfs_link_file(&package_path).is_ok() {

        info!("start downloading archive {} from LFS", package_filename);
        lfs::resolve_lfs_link(
            remote.parse().unwrap(),
            &package_path,
            Some(&cwd_package_path),
        ).map_err(CommandError::IO)?;
    } else {
        fs::copy(package_path, cwd_package_path).map_err(CommandError::IO)?;
    }

    // ? FIXME: reset back to HEAD?

    Ok(true)
}
fn find_repo_by_refspec(refspec : &String) -> Result<Option<git2::Repository>, CommandError> {
    let dot_gpm_dir = get_or_init_dot_gpm_dir().map_err(CommandError::IO)?;
    let source_file_path = dot_gpm_dir.to_owned().join("sources.list");
    let file = fs::File::open(source_file_path)?;

    for line in io::BufReader::new(file).lines() {
        let line = line.unwrap();

        debug!("searching for refspec {} in repository {}", refspec, line);

        let path = remote_url_to_cache_path(&line)?;
        let repo = git2::Repository::open(path).map_err(CommandError::Git)?;

        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();
        repo.set_head("refs/heads/master")?;
        repo.checkout_head(Some(&mut builder))?;

        let oid = repo.refname_to_id(&refspec);

        if oid.is_ok() {
            return Ok(Some(repo));
        }
    }

    Ok(None)
}

fn find_or_init_repo(
    remote : Option<String>,
    package : &String,
    version : &String
) -> Result<Option<git2::Repository>, CommandError> {

    match remote {
        Some(remote) => {
            let (repo, is_new_repo) = get_or_init_repo(&remote)?;

            if !is_new_repo {
                pull_repo(&repo).map_err(CommandError::Git)?;
            }

            Ok(Some(repo))
        },
        None => {
            let refspec = format!("refs/tags/{}/{}", package, version);
        
            debug!("no specific remote provided: searching for refspec {}", refspec);
            find_repo_by_refspec(&refspec)
        },
    }
}

fn install_command(
    remote : Option<String>,
    package : &String,
    version : &String,
    prefix : &path::Path,
    force : bool,
) -> Result<bool, CommandError> {
    info!("run install command for package {} at version {}", package, version);

    // ! FIXME: search in all repos if there is no remote provided

    let repo = match find_or_init_repo(remote, package, version)? {
        Some(repo) => repo,
        None => panic!("package was not found in any repository"),
    };
    let remote = repo.find_remote("origin")?.url().unwrap().to_owned();

    info!("package found in repository {}", remote);

    let refspec = format!("refs/tags/{}/{}", package, version);
    let oid = repo.refname_to_id(&refspec).map_err(CommandError::Git)?;
    let mut builder = git2::build::CheckoutBuilder::new();
    builder.force();

    debug!("move repository HEAD to tag {}/{}", package, version);
    repo.set_head_detached(oid).map_err(CommandError::Git)?;
    repo.checkout_head(Some(&mut builder)).map_err(CommandError::Git)?;

    let workdir = repo.workdir().unwrap();
    let package_filename = format!("{}.zip", package);
    let package_path = workdir.join(package).join(&package_filename);

    if lfs::parse_lfs_link_file(&package_path).is_ok() {
        let tmp_dir = tempdir().map_err(CommandError::IO)?;
        let tmp_package_path = tmp_dir.path().to_owned().join(&package_filename);

        info!("start downloading archive {} from LFS", package_filename);
        lfs::resolve_lfs_link(
            remote.parse().unwrap(),
            &package_path,
            Some(&tmp_package_path),
        ).map_err(CommandError::IO)?;
        extract_package(&tmp_package_path, &prefix, force);
    } else {
        warn!("package {} does not use LFS", package);
        extract_package(&package_path, &prefix, force);
    }

    // ? FIXME: reset back to HEAD?

    Ok(true)
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

fn extract_package(path : &path::Path, prefix : &path::Path, force : bool) {
    let file = fs::File::open(&path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut num_extracted_files = 0;
    let mut num_files = 0;

    // ! FIXME: compare checksums to know if we're actually upgrading/making changes

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = prefix.to_owned().join(file.sanitized_name());

        num_files += 1;

        if outpath.exists() && !force {
            info!(
                "file {} not extracted: path already exist, use --force to override",
                outpath.as_path().display()
            );
            continue;
        }

        num_extracted_files += 1;

        if (&*file.name()).ends_with('/') {
            fs::create_dir_all(&outpath).unwrap();
            info!("file {} extracted to \"{}\"", i, outpath.as_path().display());
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p).unwrap();
                }
            }
           
            let mut outfile = fs::File::create(&outpath).unwrap();
            
            io::copy(&mut file, &mut outfile).unwrap();

            info!(
                "file {} extracted to \"{}\" ({} bytes)",
                i,
                outpath.as_path().display(),
                file.size()
            );
        }
    }

    info!("{} extracted files, {} files total", num_extracted_files, num_files);
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

            debug!("authenticate with user {} and private key located in {}", user, k);
            git2::Cred::ssh_key(user, None, std::path::Path::new(&k), None)
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

fn get_or_init_repo(remote : &String) -> Result<(git2::Repository, bool), CommandError> {
    let path = remote_url_to_cache_path(remote)?;

    if path.exists() {
        debug!("use existing repository already in cache {}", path.to_str().unwrap());
        return Ok((git2::Repository::open(path)?, false));
    }

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
        let package_and_version : Vec<&str> = url.fragment().unwrap().split("/").collect();
        let repository = format!(
            "{}://{}{}",
            url.scheme(),
            url.with_default_port(default_port).unwrap(),
            url.path(),
        );

        return Ok((
            Some(repository),
            String::from(package_and_version[0]),
            String::from(package_and_version[1])
        ));
    }

    let parts : Vec<&str> = url_or_refspec.split("/").collect();

    Ok((
        None,
        parts[0].to_string(),
        parts[1].to_string(),
    ))
}

fn main() {
    pretty_env_logger::init_custom_env("GPM_LOG");

    let matches = App::new("gpm")
        .about("Git-based package manager.")
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
                    error!("package repositories have not been updated")
                }
            },
            Err(e) => error!("could not update repositories: {}", e),
        }
    }

    if let Some(_matches) = matches.subcommand_matches("clean") {
        match clean_command() {
            Ok(success) => {
                if success {
                    info!("cache successfully cleaned")
                } else {
                    error!("cache has not been cleaned")
                }
            },
            Err(e) => error!("could not clean cache: {}", e),
        }
    }

    if let Some(matches) = matches.subcommand_matches("install") {
        let force = matches.is_present("force");
        let prefix = path::Path::new(matches.value_of("prefix").unwrap());

        if !prefix.exists() {
            panic!("path {} (passed via --prefix) does not exist", prefix.to_str().unwrap());
        }
        if !prefix.is_dir() {
            panic!("path {} (passed via --prefix) is not a directory", prefix.to_str().unwrap());
        }

        let package = String::from(matches.value_of("package").unwrap());
        let (repo, package, version) = parse_package_uri(&package)
            .expect("unable to parse package URI");

        if repo.is_some() {
            debug!("parsed package URI: repo = {}, package = {}, version = {}", repo.to_owned().unwrap(), package, version);
        } else {
            debug!("parsed package: package = {}, version = {}", package, version);
        }

        match install_command(repo, &package, &version, &prefix, force) {
            Ok(_bool) => info!("package {} successfully installed at version {} in {}", package, version, prefix.display()),
            Err(e) => error!("could not install package \"{}\" with version {}: {}", package, version, e),
        }
    }

    if let Some(matches) = matches.subcommand_matches("download") {
        let force = matches.is_present("force");
        let package = String::from(matches.value_of("package").unwrap());
        let (repo, package, version) = parse_package_uri(&package)
            .expect("unable to parse package URI");

        if repo.is_some() {
            debug!("parsed package URI: repo = {}, package = {}, version = {}", repo.to_owned().unwrap(), package, version);
        } else {
            debug!("parsed package: package = {}, version = {}", package, version);
        }

        match download_command(repo, &package, &version, force) {
            Ok(success) => {
                if success {
                    info!("package {} successfully downloaded at version {}", package, version);
                } else {
                    info!("package {} has not been downloaded", package);
                }
            },
            Err(e) => error!("could not download package \"{}\" with version {}: {}", package, version, e),
        }
    }
}

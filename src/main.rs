extern crate clap; 
use clap::{App, Arg};

use std::io;

#[macro_use]
extern crate log;

extern crate pretty_env_logger;

use std::fmt;

extern crate git2;

use std::path;
use std::ops::Deref;
use std::env;
use std::fs;

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

fn download_command(
    cache : &path::Path,
    remote : &String,
    package : &String,
    version : &String,
    force : bool,
) -> Result<bool, CommandError> {
    info!("run download command for package {} at version {}", package, version);

    let (repo, is_new_repo) = get_or_init_repo(&cache, &remote).map_err(CommandError::Git)?;

    if !is_new_repo {
        pull_repo(&repo).map_err(CommandError::Git)?;
    }

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

fn install_command(
    cache : &path::Path,
    remote : &String,
    package : &String,
    version : &String,
    prefix : &path::Path,
    force : bool,
) -> Result<bool, CommandError> {
    info!("run install command for package {} at version {}", package, version);

    let (repo, is_new_repo) = get_or_init_repo(&cache, &remote).map_err(CommandError::Git)?;

    if !is_new_repo {
        pull_repo(&repo).map_err(CommandError::Git)?;
    }

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

fn init_cache_dir() -> Result<std::path::PathBuf, io::Error> {
    let cache = std::env::home_dir().unwrap().join(".gpm").join("cache");

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

fn get_or_init_repo(cache : &std::path::Path, remote : &String) -> Result<(git2::Repository, bool), git2::Error> {
    let data_url = match Url::parse(remote) {
        Ok(data_url) => data_url,
        Err(e) => panic!("failed to parse url: {}", e),
    };
    let path = cache.deref().join(data_url.host_str().unwrap()).join(&data_url.path()[1..]);

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
        Err(e) => Err(e)
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

fn parse_package_uri(url : &String) -> Result<(String, String, String), url::ParseError> {
    let url : Url = url.parse()?;
    let package_and_version : Vec<&str> = url.fragment().unwrap().split("/").collect();
    let repository = format!(
        "{}://{}{}",
        url.scheme(),
        url.with_default_port(default_port).unwrap(),
        url.path(),
    );

    Ok((repository, String::from(package_and_version[0]), String::from(package_and_version[1])))
}

fn main() {
    pretty_env_logger::init_custom_env("GPM_LOG");

    let cache = match init_cache_dir() {
        Ok(cache) => cache,
        Err(e) => panic!("failed to initialize cache directory: {}", e),
    };

    let matches = App::new("gpm")
        .about("Git-based package manager.")
        .arg(Arg::with_name("command")
            .help("the command to run")
            .value_names(&["install", "download"])
            .index(1)
            .requires("package")
            .required(true)
        )
        .arg(Arg::with_name("package")
            .help("the package URI")
            .index(2)
        )
        .arg(Arg::with_name("prefix")
            .help("the prefix to the package install path")
            .default_value("/")
            .long("--prefix")
            .required(false)
        )
        .arg(Arg::with_name("force")
            .help("replace existing files")
            .long("--force")
            .takes_value(false)
            .required(false)
        )
        .get_matches();

    let force = matches.is_present("force");
    let prefix = path::Path::new(matches.value_of("prefix").unwrap());

    if !prefix.exists() {
        panic!("path {} (passed via --prefix) does not exist", prefix.to_str().unwrap());
    }
    if !prefix.is_dir() {
        panic!("path {} (passed via --prefix) is not a directory", prefix.to_str().unwrap());
    }

    if matches.value_of("command").unwrap() == "install" {
        let package = String::from(matches.value_of("package").unwrap());
        let (repo, package, version) = parse_package_uri(&package)
            .expect("unable to parse package URI");

        debug!("parsed package URI: repo = {}, package = {}, version = {}", repo, package, version);

        match install_command(&cache, &repo, &package, &version, &prefix, force) {
            Ok(_bool) => info!("package {} successfully installed at version {} in {}", package, version, prefix.display()),
            Err(e) => error!("could not install package \"{}\" with version {}: {}", package, version, e),
        }
    } else if matches.value_of("command").unwrap() == "download" {
        let package = String::from(matches.value_of("package").unwrap());
        let (repo, package, version) = parse_package_uri(&package)
            .expect("unable to parse package URI");

        debug!("parsed package URI: repo = {}, package = {}, version = {}", repo, package, version);

        match download_command(&cache, &repo, &package, &version, force) {
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

extern crate clap; 
use clap::{App, Arg};

extern crate tar;

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

fn install_command(
    cache : &path::Path,
    remote : &String,
    package : &String,
    version : &String,
) -> Result<bool, CommandError> {
    debug!("run install command for package {} at version {}", package, version);

    let repo = get_or_init_repo(&cache, &remote).map_err(CommandError::Git)?;
    let refspec = format!("refs/tags/{}/{}", package, version);
    let oid = repo.refname_to_id(&refspec).map_err(CommandError::Git)?;
    let mut builder = git2::build::CheckoutBuilder::new();
    builder.force();

    debug!("move repository HEAD to tag {}/{}", package, version);
    repo.set_head_detached(oid).map_err(CommandError::Git)?;
    repo.checkout_head(Some(&mut builder)).map_err(CommandError::Git)?;

    let paths = fs::read_dir(repo.workdir().unwrap()).unwrap();

    debug!("explore repository to resolve LFS links");
    for path in paths {
        let p = path.unwrap().path();
        if p.to_str().unwrap().ends_with(".tar.gz") {
            lfs::resolve_lfs_link(remote.parse().unwrap(), &p).map_err(CommandError::IO)?;
        }
    }
    
    // ! FIXME: resolve the LFS link into a temp file
    // ! FIXME: extra the temp file

    // ? FIXME: reset back to HEAD?

    Ok(true)
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

fn get_or_init_repo(cache : &std::path::Path, remote : &String) -> Result<git2::Repository, git2::Error> {
    let data_url = match Url::parse(remote) {
        Ok(data_url) => data_url,
        Err(e) => panic!("failed to parse url: {}", e),
    };
    let path = cache.deref().join(data_url.host_str().unwrap()).join(&data_url.path()[1..]);

    if path.exists() {
        debug!("use existing repository already in cache {}", path.to_str().unwrap());
        return git2::Repository::open(path);
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
    
    match builder.clone(remote, &path) {
        Ok(r) => {
            debug!("repository cloned");

            Ok(r)
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
    pretty_env_logger::init();

    let cache = match init_cache_dir() {
        Ok(cache) => cache,
        Err(e) => panic!("failed to initialize cache directory: {}", e),
    };

    let matches = App::new("gpm")
        .about("Git-based package manager.")
        .arg(Arg::with_name("command")
            .help("the command to run")
            .index(1)
            .requires("package")
            .required(true)
        )
        .arg(Arg::with_name("package")
            .help("the package URI")
            .index(2)
        )
        .get_matches();

    if matches.value_of("command").unwrap() == "install" {
        let package = String::from(matches.value_of("package").unwrap());
        let (repo, package, version) = parse_package_uri(&package)
            .expect("unable to parse package URI");

        debug!("parsed package URI: repo = {}, package = {}, version = {}", repo, package, version);

        match install_command(&cache, &repo, &package, &version) {
            Ok(_bool) => trace!("successfully installed packaged {}", package),
            Err(e) => error!("could not install package \"{}\" with version {}: {}", package, version, e),
        }
    }
}

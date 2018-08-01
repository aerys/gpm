use std::fs;
use std::path;
use std::io;

use std::io::prelude::*;

use git2;

use gpm;
use gpm::error::CommandError;

extern crate indicatif;
use indicatif::{ProgressBar, ProgressStyle};

extern crate url;
use url::{Url};

pub fn get_git_credentials_callback(
    remote : &String
) -> impl Fn(&str, Option<&str>, git2::CredentialType) -> Result<git2::Cred, git2::Error>
{
    let url : Url = remote.parse().unwrap();
    let host = String::from(url.host_str().unwrap());

    move |_user: &str, _user_from_url: Option<&str>, _cred: git2::CredentialType| -> Result<git2::Cred, git2::Error> {
        let user = _user_from_url.unwrap_or("git");

        if _cred.contains(git2::CredentialType::USERNAME) {
            return git2::Cred::username(user);
        }

        let (key, passphrase) = gpm::ssh::get_ssh_key_and_passphrase(&host);
        let (has_pass, passphrase) = match passphrase {
            Some(p) => (true, p),
            None => (false, String::new()),
        };

        let key = match key {
            Some(k) => k,
            None => panic!("failed authentication for repository {}", &host),
        };

        git2::Cred::ssh_key(user, None, &key, if has_pass { Some(passphrase.as_str()) } else { None })
    }
}

pub fn pull_repo(repo : &git2::Repository) -> Result<(), git2::Error> {
    info!("fetching changes for repository {}", repo.workdir().unwrap().display());

    let mut callbacks = git2::RemoteCallbacks::new();
    let mut origin_remote = repo.find_remote("origin")?;
    callbacks.credentials(gpm::git::get_git_credentials_callback(&String::from(origin_remote.url().unwrap())));

    let mut opts = git2::FetchOptions::new();
    opts.remote_callbacks(callbacks);
    opts.download_tags(git2::AutotagOption::All);

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

pub fn get_or_clone_repo(remote : &String) -> Result<(git2::Repository, bool), CommandError> {
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
    callbacks.credentials(gpm::git::get_git_credentials_callback(remote));

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

pub fn remote_url_to_cache_path(remote : &String) -> Result<path::PathBuf, CommandError> {
    let cache = gpm::file::get_or_init_cache_dir().map_err(CommandError::IO)?;
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

pub fn revision_to_refspec(
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

pub fn find_or_init_repo(
    remote : Option<String>,
    package: &String,
    revision : &String,
) -> Result<Option<(git2::Repository, String)>, CommandError> {

    match remote {
        Some(remote) => {
            let (repo, is_new_repo) = gpm::git::get_or_clone_repo(&remote)?;

            if !is_new_repo {
                gpm::git::pull_repo(&repo).map_err(CommandError::Git)?;
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

pub fn find_repo_by_package_and_revision(
    package : &String,
    revision : &String,
) -> Result<Option<(git2::Repository, String)>, CommandError> {
    let dot_gpm_dir = gpm::file::get_or_init_dot_gpm_dir().map_err(CommandError::IO)?;
    let source_file_path = dot_gpm_dir.to_owned().join("sources.list");
    let file = fs::File::open(source_file_path)?;
    let mut remotes = Vec::new();

    for line in io::BufReader::new(file).lines() {
        let line = String::from(line.unwrap().trim());

        remotes.push(line);
    }

    let pb = ProgressBar::new(remotes.len() as u64);
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} [{elapsed_precise}] ({pos}/{len}) {wide_msg}"));
    pb.set_message(&format!("looking for {} at revision {}", &package, &revision));
    pb.set_position(0);
    pb.enable_steady_tick(200);

    for remote in remotes {
        debug!("searching for revision {} in repository {}", revision, remote);

        let path = gpm::git::remote_url_to_cache_path(&remote)?;
        let repo = git2::Repository::open(path).map_err(CommandError::Git)?;

        pb.inc(1);
        pb.set_message(&format!("looking for {}={} in {}", &package, &revision, &remote));

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
                    pb.finish_with_message(&format!("found {}={} (={}) in {}", &package, &revision, &refspec, &remote));
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

pub fn package_archive_is_in_repo(repo : &git2::Repository, package : &String) -> bool {
    let archive_filename = format!("{}.tar.gz", &package);
    let mut path = repo.workdir().unwrap().to_owned();

    path.push(package);
    path.push(archive_filename);

    return path.exists();
}

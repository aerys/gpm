use std::fs;
use std::path;
use std::io;

use std::io::prelude::*;

use git2;

use indicatif::{ProgressBar, ProgressStyle};

use url::{Url};

use crypto_hash::{Hasher, Algorithm};

use crate::gpm;
use crate::gpm::command::{CommandError};
use crate::gpm::package::Package;

pub fn get_git_credentials_callback(
    remote : &String
) -> impl Fn(&str, Option<&str>, git2::CredentialType) -> Result<git2::Cred, git2::Error>
{
    let url : Url = remote.parse().unwrap();
    let host = String::from(url.host_str().unwrap());

    move |_user: &str, user_from_url: Option<&str>, cred: git2::CredentialType| -> Result<git2::Cred, git2::Error> {
        trace!("entering git credentials callback");

        let user = user_from_url.unwrap_or("git");

        if cred.contains(git2::CredentialType::USERNAME) {
            debug!("using username from URI");
            return git2::Cred::username(user);
        }

        debug!("using username from URI");
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
    trace!("setup git credentials callback");
    callbacks.credentials(gpm::git::get_git_credentials_callback(&String::from(origin_remote.url().unwrap())));

    let oid = repo.refname_to_id("refs/remotes/origin/master")?;
    let object = repo.find_object(oid, None)?;
    trace!("reset master to HEAD");
    repo.reset(&object, git2::ResetType::Hard, None)?;

    let mut builder = git2::build::CheckoutBuilder::new();
    builder.force();
    repo.set_head("refs/heads/master")?;
    trace!("checkout head");
    repo.checkout_head(Some(&mut builder))?;

    debug!("reset head to master");
    
    let mut opts = git2::FetchOptions::new();
    opts.remote_callbacks(callbacks);

    origin_remote.fetch(&["master"], Some(&mut opts), None)?;

    debug!("fetched changes");

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
    trace!("setup git credentials callback");
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
        Err(e) => {
            error!("{:?}", e);
            dbg!(&e);
            Err(CommandError::Git(e))
        }
    }
}

pub fn remote_url_to_cache_path(remote : &String) -> Result<path::PathBuf, CommandError> {
    let cache = gpm::file::get_or_init_cache_dir().map_err(CommandError::IO)?;
    let hash = {
        let mut hasher = Hasher::new(Algorithm::SHA256);

        hasher.write(remote.as_bytes()).unwrap();

        hasher.finish()
            .into_iter()
            .fold(String::new(), |s : String, i| { s + format!("{:02x}", i).as_str() })
    };

    let mut path = path::PathBuf::new();
    path.push(cache);
    path.push(hash);

    Ok(path)
}

pub fn find_or_init_repo(
    package: &Package,
) -> Result<Option<(git2::Repository, String)>, CommandError> {

    match package.remote() {
        Some(remote) => {
            let (repo, is_new_repo) = gpm::git::get_or_clone_repo(&remote)?;

            if !is_new_repo {
                gpm::git::pull_repo(&repo).map_err(CommandError::Git)?;
            }

            match package.find(&repo) {
                Some(refspec) => match find_package_tag(package, &repo, &refspec)? {
                    Some(tag_refspec) => {
                        println!(
                            "  Found:\n    {}{}\n  in:\n    {}\n  at refspec:\n    {}\n  tagged as:\n    {}",
                            gpm::style::package_name(package.name()),
                            gpm::style::package_extension(&String::from(".tar.gz")),
                            gpm::style::remote_url(&remote),
                            gpm::style::refspec(&refspec),
                            gpm::style::refspec(&tag_refspec),
                        );

                        Ok(Some((repo, tag_refspec)))
                    },
                    None => {
                        println!(
                            "  Found:\n    {}{}\n  in:\n    {}\n  at refspec:\n    {}",
                            gpm::style::package_name(package.name()),
                            gpm::style::package_extension(&String::from(".tar.gz")),
                            gpm::style::remote_url(&remote),
                            gpm::style::refspec(&refspec),
                        );

                        Ok(Some((repo, refspec)))
                    },
                },
                None => Err(CommandError::NoMatchingVersion())
            }
        },
        None => {
            debug!("no specific remote provided: searching");

            find_repo_by_package_and_revision(&package)
        },
    }
}

fn commit_to_tag_name(repo : &git2::Repository, commit_id : &git2::Oid) -> Result<Option<String>, git2::Error> {
    let tag_names = repo.tag_names(None)?;

    for tag_name in tag_names.iter() {
        let tag_name = tag_name.unwrap();
        let tag = repo.find_reference(&format!("refs/tags/{}", &tag_name))?;
        match tag.peel(git2::ObjectType::Commit) {
            Ok(c) => if c.as_commit().unwrap().id() == *commit_id { return Ok(Some(String::from(tag_name))); },
            _ => continue,
        }
    }

    Ok(None)
}

fn diff_tree_has_path(path : &path::Path, repo : &git2::Repository, tree : &git2::Tree) -> bool {
    let mut found = false;
    let mut found_binary = false;
    let diff = repo.diff_tree_to_workdir_with_index(Some(&tree), None).unwrap();
    // iterate over all the changes in the diff
    diff.foreach(&mut |a, _| {
        // when using LFS, the changed file is *not* a binary file
        if a.new_file().path().unwrap() == path {
            found = true;
        }
        true
    } , Some(&mut |a, _| {
        // when *not* using LFS, the changed file *is* a binary file
        if a.new_file().path().unwrap() == path {
            found_binary = true;
        }
        true
    }), None, None).unwrap();

    return found || found_binary;
}

pub fn find_last_commit_id(
    path : &path::Path,
    repo : &git2::Repository
) -> Result<git2::Oid, git2::Error> {
    let mut commit = repo
        .head()?
        .peel_to_commit()?;
    let mut previous_commit = commit.clone();

    loop {
        let tree = commit.tree().unwrap();

        if diff_tree_has_path(&path, &repo, &tree) {
            debug!("package last modified by commit {:?}", previous_commit);

            return Ok(previous_commit.id());
        }

        let parent = commit.parent(0)?;

        previous_commit = commit;
        commit = parent;
    }
}

pub fn find_repo_by_package_and_revision(
    package : &Package,
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
        .template("{spinner:.green} [{elapsed_precise}] ({pos}/{len}) {msg}"));
    pb.set_position(0);
    pb.enable_steady_tick(200);

    for remote in remotes {
        debug!("searching in repository {}", remote);

        let path = gpm::git::remote_url_to_cache_path(&remote)?;
        let repo = git2::Repository::open(path).map_err(CommandError::Git)?;

        pb.inc(1);
        pb.set_message(&remote);

        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();
        repo.set_head("refs/heads/master")?;
        repo.checkout_head(Some(&mut builder))?;

        match package.find(&repo) {
            Some(refspec) => {
                debug!("found with refspec {}", refspec);

                pb.finish();

                match find_package_tag(package, &repo, &refspec)? {
                    Some(tag_name) => {
                        println!(
                            "    Found:\n      {}{}\n    in:\n      {}\n    at refspec:\n      {}\n    tagged as:\n      {}",
                            gpm::style::package_name(package.name()),
                            gpm::style::package_extension(&String::from(".tar.gz")),
                            gpm::style::remote_url(&remote),
                            gpm::style::refspec(&refspec),
                            gpm::style::refspec(&tag_name),
                        );
                        
                        return Ok(Some((repo, tag_name)));
                    },
                    None => {
                        println!(
                            "    Found:\n      {}{}\n    in:\n      {}\n    at refspec:\n      {}",
                            gpm::style::package_name(package.name()),
                            gpm::style::package_extension(&String::from(".tar.gz")),
                            gpm::style::remote_url(&remote),
                            gpm::style::refspec(&refspec),
                        );

                        return Ok(Some((repo, refspec)));
                    },
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

fn find_package_tag(
    package: &Package,
    repo: &git2::Repository,
    refspec: &String,
) -> Result<Option<String>, CommandError> {
    let mut builder = git2::build::CheckoutBuilder::new();
    builder.force();
    repo.set_head(&refspec)?;
    repo.checkout_head(Some(&mut builder))?;

    if package.archive_is_in_repository(&repo) {
        debug!("package archive found in refspec {}", &refspec);

        let package_commit_id = find_last_commit_id(
            &package.get_archive_path(None),
            &repo,
        ).map_err(CommandError::Git)?;

        match commit_to_tag_name(&repo, &package_commit_id).map_err(CommandError::Git)? {
            Some(tag_name) => {
                return Ok(Some(format!("refs/tags/{}", tag_name)));
            },
            // every published package version should be tagged, so this match should "never" happen...
            None => (),
        }
    }

    return Ok(None);
}

# GPM

A statically linked, native, platform agnostic Git-based package manager written in Rust.

<!-- TOC depthFrom:2 -->

- [Build](#build)
    - [Development build](#development-build)
    - [Release (static) build](#release-static-build)
- [Authentication](#authentication)
- [Package reference formatting](#package-reference-formatting)
    - [Refspec](#refspec)
    - [URI](#uri)
- [Logging](#logging)
- [Commands](#commands)
    - [`update`](#update)
    - [`clean`](#clean)
    - [`install`](#install)
    - [`download`](#download)
- [FAQ](#faq)
    - [Why GPM?](#why-gpm)
    - [Why Git? Why not just `curl` or `wget` or whatever?](#why-git-why-not-just-curl-or-wget-or-whatever)
    - [But Git does not like large binary files!](#but-git-does-not-like-large-binary-files)

<!-- /TOC -->

## Build

### Development build

Dependencies:

* OpenSSL


```bash
cargo build
```

### Release (static) build

Dependencies:

* Docker

```bash
docker run \
    --rm -it \
    -v "$(pwd)":/home/rust/src \
    -v "/home/${USER}/.cargo":/home/rust/.cargo \
    ekidd/rust-musl-builder \
    cargo build --release --target x86_64-unknown-linux-musl
```

## Authentication

If the repository is "public", then no authentication should be required.

Otherwise, for now, only authentication through a passphrase-less SSH private key is supported.
The path to that SSH private key must be set in the `GPM_SSH_KEY` environment variable.

## Package reference formatting

### Refspec

A package can be referenced using a Git refspec.
The best practice is to use a Git tag with the following format:

`${name}/${version}`

where:

* `name` is the name of the package,
* `version` is the version of the package.

Example: `my-package/2.0`

In this case, `gpmh` will look for that refspec in all the repositories listed in `~/.gpm/sources.list`
and available in the cache.

For such package reference to be found, you *must* make sure:
* the repository where that package is stored is listed in `~/.gpm/sources.list`,
* the cache has been populated by calling `gpm update`.

### URI

A package can also be referenced using a full Git URI formatted like this:

`${remote-uri}#${refspec}`

where:

* `remote-uri` is the full URI to the Git remote,
* `refspec` is the refspec for the package (usually a Git tag).

Example:

`ssh://github.com/my/awesome-packages.git#app/2.0`

In this case, `gpm` will clone the corresponding Git repository and look for the package there.
`gpm` will look for the specified package *only* in the specified repository.

## Logging

By default, `gpm` will echo nothing on stdout.
Logs can be enable by setting the `GPM_LOG` environment variable to one of the following values:

* `trace`
* `debug`
* `info`
* `warn`
* `error`

Logs can be *very* verbose. So it's best to keep only the `gpm` and `gitlfs` module logs.
For example:

```bash
export GPM_LOG=gpm=debug,gitlfs=debug
```

## Commands

### `update`

Update the cache to feature the latest revision of each repository listed in `~/.gpm/sources.list`.

Example:

```bash
# first add at least one remote
echo "ssh://github.com/my/awesome-packages.git" >> ~/.gpm/sources.list
echo "ssh://github.com/my/other-packages.git" >> ~/.gpm/sources.list
# ...
# then you can run an update:
gpm update
```

### `clean`

Clean the cache. The cache is located in `~/.gpm/cache`.
Cache can be rebuilt using the `update` command.

```bash
gpm clean
```

### `install`

Download and install a package.

Example:

```bash
# install the "app" package at version 2.0 from repository ssh://github.com/my/awesome-packages.git
# in the /var/www/app folder
gpm install ssh://github.com/my/awesome-packages.git#app/2.0 \
    --prefix /var/www/app
```

```bash
# assuming the repository ssh://github.com/my/awesome-packages.git is in ~/.gpm/sources.list
# and the cache has been updated using `gpm update`
gpm install app/2.0 --prefix /var/www/app
```

### `download`

Download a package in the current working directory.

Example:

```bash
# install the "app" package at version 2.0 from repository ssh://github.com/my/awesome-packages.git
# in the /var/www/app folder
gpm download ssh://github.com/my/awesome-packages.git#app/2.0 \
    --prefix /var/www/app
```

```bash
# assuming the repository ssh://github.com/my/awesome-packages.git is in ~/.gpm/sources.list
# and the cache has been updated using `gpm update`
gpm download app/2.0 --prefix /var/www/app
```

## FAQ

### Why GPM?

GPM means "Git-based Package Manager".

The main motivation is to have a platform-agnostic package manager, mainly aimed at distributing binary packages as archives.
GPM can be used to leverage any Git repository as a package repository.

Platforms like GitLab and GitHub are then very handy to manage such package archives, permissions, etc...

GPM is also available as an all-in-one static binary.
It can be leveraged to download some packages that will be used to bootrasp a more complex provisioing process.

### Why Git? Why not just `curl` or `wget` or whatever?

GPM aims at leveraging the Git ecosystem and features.

Git is great to manage revisions. So it's great at managing package versions!
For example, Git is also used by the Docker registry to store Docker images.

Git also has a safe and secured native authentication/authorization strategy through SSH.
With GitLab, you can safely setup [deploy keys](https://docs.gitlab.com/ce/ssh/README.html#deploy-keys) to give a read-only access to your packages.

### But Git does not like large binary files!

Yes. Cloning a repository full of large binary files can take a lot of time and space.
You certainly don't want to checkout all the versions of all your packages everytime you want to install one of them.

That's why you should use [git-lfs](https://git-lfs.github.com/) for your GPM repositories.

Thanks to [git-lfs](https://git-lfs.github.com/), GPM will download the a actual binary package only when it is required.

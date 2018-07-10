# GPM

A statically linked, native, platform agnostic Git-based package manager written in Rust.

<!-- TOC depthFrom:2 -->

- [1. Getting started](#1-getting-started)
    - [1.1. Creating a package repository](#11-creating-a-package-repository)
    - [1.2. Publishing your first package](#12-publishing-your-first-package)
    - [1.3. Installing your first package](#13-installing-your-first-package)
- [2. Build](#2-build)
    - [2.1. Development build](#21-development-build)
    - [2.2. Release (static) build](#22-release-static-build)
- [3. Authentication](#3-authentication)
- [4. Package reference formatting](#4-package-reference-formatting)
    - [4.1. Refspec](#41-refspec)
    - [4.2. URI](#42-uri)
- [5. Logging](#5-logging)
- [6. Commands](#6-commands)
    - [6.1. `update`](#61-update)
    - [6.2. `clean`](#62-clean)
    - [6.3. `install`](#63-install)
    - [6.4. `download`](#64-download)
- [7. FAQ](#7-faq)
    - [7.1. Why GPM?](#71-why-gpm)
    - [7.2. Why Git? Why not just `curl` or `wget` or whatever?](#72-why-git-why-not-just-curl-or-wget-or-whatever)
    - [7.3. But Git does not like large binary files!](#73-but-git-does-not-like-large-binary-files)

<!-- /TOC -->

## 1. Getting started

### 1.1. Creating a package repository

* Create a [git-lfs](https://git-lfs.github.com/) enabled Git repository, for example a GitHub or GitLab repository.
* Clone this repository on your local computer: ̀`git clone ssh://path.to/my/package-repository.git && cd package-repository`.
* [Install git-lfs](https://github.com/git-lfs/git-lfs/wiki/Installation).
* Enable [git-lfs](https://git-lfs.github.com/) tracking for `*.zip` files: `git lfs track "*.zip"`.
* Add, commit and push `.gitattributes`: `git add .gitattributes && git commit -a -m "Enable git-lfs." && git push`.

Voilà! You're all set to publish your first package!

### 1.2. Publishing your first package

In this example, we're going to create a simple `hello-world` package and publish it.

* Make sure you are at the root of the package repository created in the previous section.
* Create and enter the package directory: `mkdir hello-world && cd hello-world`.
* Create the `hello-world.sh` script: `echo "#/bin/sh\necho 'Hello World!'" > hello-world.sh`.
* Create your package archive: `zip hello-world.zip hello-world.sh`.
* Add and commit your package archive: `git add hello-world.zip && git commit hello-world.zip -m "Publish hello-world version 1.0"`.
* Tag your package release with a specific version number: `git tag hello-world/1.0`.
* Push your new package: `git push --tags`

Your `hello-world/1.0` package is now stored in your package repository and can be installed using `gpm`!

### 1.3. Installing your first package

* Download or build `gpm`.
* Add your package repository to the `gpm` sources: `mkdir -p ~/.gpm/sources.list && echo "ssh://path.to/my/package-repository.git" >> ~/.gpm/sources.list`.
* Update the `gpm` cache: `gpm update`.u
* Install your package: `gpm install hello-world/1.0 --prefix ~/`.

Your `hello-world/1.0` package is now installed and you can run it with `sh ~/hello-world.sh`.

## 2. Build

### 2.1. Development build

Dependencies:

* OpenSSL


```bash
cargo build
```

### 2.2. Release (static) build

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

## 3. Authentication

If the repository is "public", then no authentication should be required.

Otherwise, for now, only authentication through a passphrase-less SSH private key is supported.
The path to that SSH private key must be set in the `GPM_SSH_KEY` environment variable.

## 4. Package reference formatting

### 4.1. Refspec

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

### 4.2. URI

A package can also be referenced using a full Git URI formatted like this:

`${remote-uri}#${refspec}`

where:

* `remote-uri` is the full URI to the Git remote,
* `refspec` is the refspec for the package (usually a Git tag).

Example:

`ssh://github.com/my/awesome-packages.git#app/2.0`

In this case, `gpm` will clone the corresponding Git repository and look for the package there.
`gpm` will look for the specified package *only* in the specified repository.

## 5. Logging

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

## 6. Commands

### 6.1. `update`

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

### 6.2. `clean`

Clean the cache. The cache is located in `~/.gpm/cache`.
Cache can be rebuilt using the `update` command.

```bash
gpm clean
```

### 6.3. `install`

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

### 6.4. `download`

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

## 7. FAQ

### 7.1. Why GPM?

GPM means "Git-based Package Manager".

The main motivation is to have a platform-agnostic package manager, mainly aimed at distributing binary packages as archives.
GPM can be used to leverage any Git repository as a package repository.

Platforms like GitLab and GitHub are then very handy to manage such package archives, permissions, etc...

GPM is also available as an all-in-one static binary.
It can be leveraged to download some packages that will be used to bootrasp a more complex provisioing process.

### 7.2. Why Git? Why not just `curl` or `wget` or whatever?

GPM aims at leveraging the Git ecosystem and features.

Git is great to manage revisions. So it's great at managing package versions!
For example, Git is also used by the Docker registry to store Docker images.

Git also has a safe and secured native authentication/authorization strategy through SSH.
With GitLab, you can safely setup [deploy keys](https://docs.gitlab.com/ce/ssh/README.html#deploy-keys) to give a read-only access to your packages.

### 7.3. But Git does not like large binary files!

Yes. Cloning a repository full of large binary files can take a lot of time and space.
You certainly don't want to checkout all the versions of all your packages everytime you want to install one of them.

That's why you should use [git-lfs](https://git-lfs.github.com/) for your GPM repositories.

Thanks to [git-lfs](https://git-lfs.github.com/), GPM will download the a actual binary package only when it is required.

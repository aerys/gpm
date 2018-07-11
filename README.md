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
- [5. Package reference formatting](#5-package-reference-formatting)
    - [5.1. Refspec](#51-refspec)
    - [5.2. URI](#52-uri)
- [4. Working with multiple package repositories](#4-working-with-multiple-package-repositories)
- [6. Logging](#6-logging)
- [7. Commands](#7-commands)
    - [7.1. `update`](#71-update)
    - [7.2. `clean`](#72-clean)
    - [7.3. `install`](#73-install)
    - [7.4. `download`](#74-download)
- [8. FAQ](#8-faq)
    - [8.1. Why GPM?](#81-why-gpm)
    - [8.2. Why Git? Why not just `curl` or `wget` or whatever?](#82-why-git-why-not-just-curl-or-wget-or-whatever)
    - [8.3. But Git does not like large binary files!](#83-but-git-does-not-like-large-binary-files)
    - [Why storing packages as ZIP archives?](#why-storing-packages-as-zip-archives)

<!-- /TOC -->

## 1. Getting started

### 1.1. Creating a package repository

1. Create a [git-lfs](https://git-lfs.github.com/) enabled Git repository, for example a GitHub or GitLab repository.
2. [Install git-lfs](https://github.com/git-lfs/git-lfs/wiki/Installation) on your local computer.
3. Clone the newly created repository on your local computer:

```bash
git clone ssh://path.to/my/package-repository.git
cd package-repository
```

4. Enable [git-lfs](https://git-lfs.github.com/) tracking for `*.zip` files:

```bash
git lfs track "*.zip"
```

5. Add, commit and push `.gitattributes`:

```bash
git add .gitattributes
git commit -a -m "Enable git-lfs."
git push
```

Voilà! You're all set to publish your first package!

### 1.2. Publishing your first package

In this example, we're going to create a simple `hello-world` package and publish it.

1. Make sure you are at the root of the package repository created in the previous section.
2. Create and enter the package directory:

```bash
mkdir hello-world && cd hello-world
```

3. Create the `hello-world.sh` script:

```bash
echo "#/bin/sh\necho 'Hello World!'" > hello-world.sh
```

4. Create your package archive:

```bash
zip hello-world.zip hello-world.sh
```

5. Add and commit your package archive:

```bash
git add hello-world.zip
git commit hello-world.zip -m "Publish hello-world version 1.0"
```

6. Tag your package release with a specific version number:

```bash
git tag hello-world/1.0
```

7. Push your new package:

```bash
git push
git push --tags
```

Your `hello-world/1.0` package is now stored in your package repository and can be installed using `gpm`!

### 1.3. Installing your first package

1. Download or build `gpm`.
2. Add your package repository to the `gpm` sources:

```bash
mkdir -p ~/.gpm/sources.list
echo "ssh://path.to/my/package-repository.git" >> ~/.gpm/sources.list
```

3. Update the `gpm` cache:

```bash
gpm update
```

4. Install your package:

```bash
gpm install hello-world/1.0 --prefix ~/
```

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

## 5. Package reference formatting

### 5.1. Refspec

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
* the repository where that package is stored is listed in `~/.gpm/sources.list` (see
[Working with multiple package repositories](#4-working-with-multiple-package-repositories)),
* the cache has been updated by calling `gpm update`.

### 5.2. URI

A package can also be referenced using a full Git URI formatted like this:

`${remote-uri}#${refspec}`

where:

* `remote-uri` is the full URI to the Git remote,
* `refspec` is the refspec for the package (usually a Git tag).

Example:

`ssh://github.com/my/awesome-packages.git#app/2.0`

In this case, `gpm` will clone the corresponding Git repository and look for the package there.
`gpm` will look for the specified package *only* in the specified repository.

## 4. Working with multiple package repositories

Specifying a full package URI might not be practical. It's simpler to specify a package
refspec and let `gpm` find it. But where should it look for it?

When you specify a package using a refspec, `gpm` will have to find the proper package
repository. It will look for this refspec in the repositories listed in `~/.gpm/sources.list`.

The following command lines will fill `sources.list` with a few (dummy) package repositories:

```bash
echo "ssh://path.to/my/package-repository.git" >> ~/.gpm/sources.list
echo "ssh://path.to/my/another-repository.git" >> ~/.gpm/sources.list
echo "ssh://path.to/my/yet-another-repository.git" >> ~/.gpm/sources.list
# ...
```

After updating `sources.list`, don't forget to call `gmp update` to update the cache.

You can then install packages using their refspec.

## 6. Logging

By default, `gpm` will echo *nothing* on stdout.

Logs can be enable by setting the `GPM_LOG` environment variable to one of the following values:

* `trace`
* `debug`
* `info`
* `warn`
* `error`

Example:

```bash
GPM_LOG=info gpm update
```

Logs can be *very* verbose. So it's best to keep only the `gpm` and `gitlfs` module logs.
For example:

```bash
GPM_LOG="gpm=debug,gitlfs=debug" gpm install hello-world/1.0
```

## 7. Commands

### 7.1. `update`

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

### 7.2. `clean`

Clean the cache. The cache is located in `~/.gpm/cache`.
Cache can be rebuilt using the `update` command.

```bash
gpm clean
```

### 7.3. `install`

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

### 7.4. `download`

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

## 8. FAQ

### 8.1. Why GPM?

GPM means "Git-based Package Manager".

The main motivation is to have a platform-agnostic package manager, mainly aimed at distributing binary packages as archives.
GPM can be used to leverage any Git repository as a package repository.

Platforms like GitLab and GitHub are then very handy to manage such package archives, permissions, etc...

GPM is also available as an all-in-one static binary.
It can be leveraged to download some packages that will be used to bootrasp a more complex provisioing process.

### 8.2. Why Git? Why not just `curl` or `wget` or whatever?

GPM aims at leveraging the Git ecosystem and features.

Git is great to manage revisions. So it's great at managing package versions!
For example, Git is also used by the Docker registry to store Docker images.

Git also has a safe and secured native authentication/authorization strategy through SSH.
With GitLab, you can safely setup [deploy keys](https://docs.gitlab.com/ce/ssh/README.html#deploy-keys) to give a read-only access to your packages.

### 8.3. But Git does not like large binary files!

Yes. Cloning a repository full of large binary files can take a lot of time and space.
You certainly don't want to checkout all the versions of all your packages everytime you want to install one of them.

That's why you should use [git-lfs](https://git-lfs.github.com/) for your GPM repositories.

Thanks to [git-lfs](https://git-lfs.github.com/), GPM will download the a actual binary package only when it is required.

### Why storing packages as ZIP archives?

Vanilla Git will compress objects. But git-lfs doesn't store objects in the actual Git
repository: they are stored "somewhere else".

To make sure we don't use too much disk space/bandwidth "somewhere else", the
package archive is stored compressed.

`gpm` is using ZIP because:
* it's the most common compression algorithms already supported by Rust;
* tools to create ZIP archives are widely available and easy to use.

`gpm` might support other compression algorithms in the future.

# GPM

A statically linked native binary, platform agnostic, Git-based package manager written in Rust.

<!-- TOC depthFrom:2 -->

- [Build](#build)
    - [Development build](#development-build)
    - [Release (static) build](#release-static-build)
- [Authentication](#authentication)
- [Commands](#commands)
    - [`install`](#install)
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

For now, only authentication through a passphrase-less SSH private key is supported.
The path to that SSH private key must be set in the `GPM_SSH_KEY` environment variable.

## Commands

### `install`

Download and install a package.

Example:

```bash
# install the "app" package at version 2.0 from repository ssh://github.com/my/awesome-packages.git
# in the /var/www/app folder
gpm install ssh://github.com/my/awesome-packages.git#app/2.0 \
    --prefix /var/www/app
```

## FAQ

### Why GPM?

GPM means "Git-based Package Manager".

The main motivation is to have a platform-agnostic package manager, mainly aimed at distributing binary packages as archives.
GPM can be used to leverage any Git repository as a package repository.

Platforms like GitLab and GitHub are then very handy to manage such package archives, permissions, etc...

GPM is also available as an all-in-one static binary.
It can be used to download some packages that will be used to bootrasp a more complex provisioing process.

### Why Git? Why not just `curl` or `wget` or whatever?

GPM aims at leveraging the Git ecosystem and features.

Git is great to manage revisions. So it's great at managing package versions!
For example, Git is also use by the Docker registry to store Docker images.

Git also has a safe and secured native authentication/authorization strategy through SSH.
With GitLab, you can safely setup [deploy keys](https://docs.gitlab.com/ce/ssh/README.html#deploy-keys) to give a read-only access to your packages.

### But Git does not like large binary files!

Yes. Cloning a repository full of large binary files can take a lot of time and space.
You certainly don't want to checkout all the versions of all your packages everytime you want to install one of them.

That's why you should use [git-lfs](https://git-lfs.github.com/) for your GPM repositories.

Thanks to [git-lfs](https://git-lfs.github.com/), GPM will download the an actual binary package only when it is are actually required.

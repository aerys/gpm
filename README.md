# GPM

[![Build status](https://travis-ci.org/aerys/gpm.svg?branch=master)](https://travis-ci.org/aerys/gpm)
[![Build status](https://ci.appveyor.com/api/projects/status/rxgs74va4o640vaa?svg=true)](https://ci.appveyor.com/project/promethe42/gpm)

A statically linked, native, platform agnostic Git-based Package Manager written in Rust.

![demo](./demo.gif)

<!-- TOC depthFrom:2 -->

- [1. Install](#1-install)
- [2. Background](#2-background)
- [3. Features](#3-features)
- [4. Security](#4-security)
- [5. Build](#5-build)
    - [5.1. Development build](#51-development-build)
    - [5.2. Release (static) build](#52-release-static-build)
- [6. Getting started](#6-getting-started)
    - [6.1. Creating a package repository](#61-creating-a-package-repository)
    - [6.2. Publishing your first package](#62-publishing-your-first-package)
    - [6.3. Installing your first package](#63-installing-your-first-package)
- [7. Authentication](#7-authentication)
- [8. Package reference notation](#8-package-reference-notation)
    - [8.1. Package name](#81-package-name)
        - [8.1.1. Shorthand notation](#811-shorthand-notation)
        - [8.1.2. URI notation](#812-uri-notation)
    - [8.2. Package version](#82-package-version)
        - [8.2.1. SemVer notation](#821-semver-notation)
        - [8.2.2. Git refspec notation](#822-git-refspec-notation)
- [9. Matching package references](#9-matching-package-references)
- [10. Working with multiple package repositories](#10-working-with-multiple-package-repositories)
- [11. Logging](#11-logging)
- [12. Commands](#12-commands)
    - [12.1. `update`](#121-update)
    - [12.2. `clean`](#122-clean)
    - [12.3. `install`](#123-install)
    - [12.4. `download`](#124-download)
- [13. Integrations](#13-integrations)
    - [13.1. Travis CI](#131-travis-ci)
    - [13.2. AppVeyor](#132-appveyor)
    - [13.3. GitLab CI](#133-gitlab-ci)
    - [13.4. GitLab Releases](#134-gitlab-releases)
    - [13.5. GitHub Releases](#135-github-releases)
- [14. FAQ](#14-faq)
    - [14.1. Why GPM?](#141-why-gpm)
    - [14.2. Why Git? Why not just `curl` or `wget` or whatever?](#142-why-git-why-not-just-curl-or-wget-or-whatever)
    - [14.3. But Git does not like large binary files!](#143-but-git-does-not-like-large-binary-files)
    - [14.4. Why storing packages as `*.tar.gz` archives?](#144-why-storing-packages-as-targz-archives)
- [15. Contributing](#15-contributing)
- [16. License](#16-license)

<!-- /TOC -->

## 1. Install

* Linux: `curl -Ls https://github.com/aerys/gpm-packages/raw/master/gpm-linux64/gpm-linux64.tar.gz | tar xvz`
* Windows:
  * cmd.exe: `curl -Ls https://github.com/aerys/gpm-packages/raw/master/gpm-windows64/gpm-windows64.tar.gz | tar xvzf -`
  * PowerShell: `$tmp = New-TemporaryFile ; Invoke-WebRequest -OutFile $tmp https://github.com/aerys/gpm-packages/raw/master/gpm-windows64/gpm-windows64.tar.gz ; tar xf $tmp`

## 2. Background

As [a software company](https://aerys.in), we use Git to manage our source code.
Thus, we wanted to **use the same Git features we know and love to manage the
packages built from that source code**.

We also use Git-enabled collaborative platforms (GitLab, GitHub, Gitea...) to:

* Build, package and deploy our code using CI/CD.
* Authenticate and authorize clients to report issues and manage projects.

And we wanted to leverage those collaborative features to **effortlessly
distribute packages to authorized users**.

**Introducing GPM: the Git-based Package Manager.**

GPM is a platform-agnostic package manager leveraging Git capabilities to
store, install and update packages. Thanks to GPM:

* Any Git repository becomes a package repository.
* Any Git-powered collaborative platform (GitLab, GitHub, Gitea...) becomes
a package management and distribution platform.

## 3. Features

* Free, open source and decentralized package management
* 100% backed by the Git protocol
* Fully compatible with the Git ecosystem (GitLab, GitHub, Gitea...):
  * commit and manage packages via your favorite Git client
  * authentication (password, private key, deploy key, deploy token...)
  * commit packages via Git in CI pipelines
  * protected package version branches/tags
  * release page integration (ex: [GPM binary package releases](https://github.com/aerys/gpm-packages/releases))
  * [Git LFS](https://git-lfs.github.com/) support
  * release notes via Git tag messages
* Distribute *any* package for *any* platform as a simple `*.tar.gz` archive
* [Semver](https://semver.org) support
* Public package repositories
* Private package repositories with authentication:
  * username/password, deploy token
  * SSH private key, deploy key
* Modern, intuitive and clean CLI interactive mode
* Non-interactive mode for automation/deployments
* Transparent SSH config (`~/.ssh/config`) discovery for authentication
* Support for all the common Git protocols (HTTP(s), SSH...)
* Security backed by standard libraries (libgit2, libssh2, OpenSSL...)
* Lightweight (<2,5Mo) fully static 0 dependency binary
* Cross-platform: Windows, Linux, Android (Termux), macOS (untested)
* One-liner installation
* Local cache to speedup package discovery and matching

## 4. Security

GPM leverages other standard well maintained open source libraries for all
sensitive operations:

* All Git operations (clone, pull...) are performed using [the Rust bindings for libgit2](https://crates.io/crates/git2).
* All SSH operations (Git LFS authentication) are performed using [the Rust bindings for libssh2](https://crates.io/crates/ssh2).
* All SSL operations (HTTPS, SSH key management) are performed using [the Rust bindings for OpenSSL](https://crates.io/crates/openssl-sys).

## 5. Build

### 5.1. Development build

Dependencies:

* OpenSSL

```bash
cargo build
```

### 5.2. Release (static) build

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

## 6. Getting started

### 6.1. Creating a package repository

1. Create a [Git LFS](https://git-lfs.github.com/) enabled Git repository, for example a GitHub or GitLab repository.
2. [Install Git LFS](https://github.com/git-lfs/git-lfs/wiki/Installation) on your local computer.
3. Clone the newly created repository on your local computer:

```bash
git clone ssh://path.to/my/package-repository.git
cd package-repository
```

4. Enable [Git LFS](https://git-lfs.github.com/) tracking for `*.tar.gz` files:

```bash
git lfs track "*.tar.gz"
```

5. Add, commit and push `.gitattributes`:

```bash
git add .gitattributes
git commit .gitattributes -m "Enable Git LFS."
git push
```

Voilà! You're all set to publish your first package!

### 6.2. Publishing your first package

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
tar -cvzf hello-world.tar.gz hello-world.sh
```

5. Add and commit your package archive:

```bash
git add hello-world.tar.gz
git commit hello-world.tar.gz -m "Publish hello-world version 0.1.0"
```

6. Tag your package release with a specific version number:

```bash
git tag hello-world/0.1.0
```

7. Push your new package:

```bash
git push
git push --tags
```

Your `hello-world/0.1.0` package is now stored in your package repository and can be installed using `gpm`!

### 6.3. Installing your first package

1. Install (or build) `gpm`.
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
gpm install hello-world=0.1.0 --prefix ~/
```

The version `0.1.0`of your `hello-world` package is now installed and you can run it with `sh ~/hello-world.sh`.

## 7. Authentication

`gpm` will behave a lot like `git` regarding authentication.

If the repository is "public", then no authentication should be required.

Otherwise, the following authentication methods are supported:

* URL encoded HTTP basic authentication (ex: `https://username:password@host.com/repo.git`);
* SSH public/private key.

If URL encoded HTTP basic authentication is used, no additional authentication is required.
Otherwise, `gpm` will assume SSH public/private key authentication is used.

If SSH public/private key authentication is used:

* If the `GPM_SSH_KEY` environment variable is set to a path that exists/is a file, then its value is used as the path to the SSH private key.
* Otherwise, if `gpm` can find the `~/.ssh/config` file, parse it and find a matching host with the `IndentityFile` option; then the corresponding
path to the SSH private key will be used.
* Otherwise, if `gpm` can find the `~/.ssh/id_rsa` file, it is used as the SSH private key.
* Otherwise, `gpm` will continue without authentication.

If the SSH private key requires a passphrase, then:

* If the `GPM_SSH_PASS` environment variable is set/not empty, it is used as the passphrase.
* Otherwise, `gpm` will prompt the user to type his passphrase.

## 8. Package reference notation

### 8.1. Package name

#### 8.1.1. Shorthand notation

This is the most trivial, obvious and simple notation: simply use the package name.

Example:

```
gpm install my-package
```

`gpm` will search by name for the specified package in all the available package
repositories. Thus, for such package reference to be found, you *must* make sure:

* The corresponding package repository remote is listed in `~/.gpm/sources.list` (see
[Working with multiple package repositories](#7-working-with-multiple-package-repositories)).
* The cache has been updated by calling `gpm update`.

#### 8.1.2. URI notation

The complete URI notation for a package is as follow:

`${remote-uri}#${package}`

* `remote-uri`: the full URI to the Git remote.
* `package`: a shorthand or `${name}=${revision}` package reference.

Example:

```
gpm install ssh://github.com/my/awesome-packages.git#my-package
```

In this case, `gpm` will clone the corresponding Git repository and look for the package there.
`gpm` will look for the specified package *only* in the specified repository.

### 8.2. Package version

#### 8.2.1. SemVer notation

The version of a package can be specified using the
[SemVer](https://semver.org/) version requirement notation:

`${package}${semver_req}`

* `package`: the name of the package (ex: `my-package`).
* `semver_req`: the semver version requirement (ex: `^0.4.2`). If not
specified, then the latest version will be installed.

It also allows parsing of `~x.y.z` and `^x.y.z` requirements as defined
at https://www.npmjs.org/doc/misc/semver.html.

Tilde requirements specify a minimal version with some updates:

```
~1.2.3 := >=1.2.3 <1.3.0
~1.2   := >=1.2.0 <1.3.0
~1     := >=1.0.0 <2.0.0
```

Caret requirements allow SemVer compatible updates to a specified verion,
`0.x` and `0.x+1` are not considered compatible, but `1.x and `1.x+1 are.

0.0.x is not considered compatible with any other version. Missing minor and
patch versions are desugared to 0 but allow flexibility for that value.

```
^1.2.3 := >=1.2.3 <2.0.0
^0.2.3 := >=0.2.3 <0.3.0
^0.0.3 := >=0.0.3 <0.0.4
^0.0   := >=0.0.0 <0.1.0
^0     := >=0.0.0 <1.0.0
```

Wildcard requirements allows parsing of version requirements of the formats
`*`, `x.*` and `x.y.*`.

```
*     := >=0.0.0
1.*   := >=1.0.0 <2.0.0
1.2.* := >=1.2.0 <1.3.0
```

Example:

```bash
gpm install my-package>=2.0.0
```

#### 8.2.2. Git refspec notation

A package version can also be set to a valid
[Git refspec](https://git-scm.com/book/en/v2/Git-Internals-The-Refspec),
such as a specific branch or a tag, using the `@` operator:

`${package}@${refspec}`

* `package` is the name of the package (ex: `my-package`).
* `refspec` is a valid Git refspec (ex: `refs/heads/my-branch` or `refs/tags/my-tag`).

## 9. Matching package references

The following section explains how `gpm` finds the package archive for a
package named `${package_name}` at version `${package_version}`.

The following pseudo code explains how GPM will find packages for a specific
version:

```
if ${package_ref} includes remote URL
    git clone URL

if ${package_ref} does not include version
    ${package_version} is set to "@master"

for each ${repository} in cache
    git checkout master
    git reset --hard

    if ${package_version} is refspec
        git checkout ${package_version}
    else # assume ${package_version} is semver
        for each ${tag} in ${repository}
            if ${tag} matches semver
                git checkout ${tag}
    
    if file ${package_name}/${package_name}.tgz exists
        if ${package_name}/${package_name}.tgz is LFS link
            download ${package_name}/${package_name}.tgz
        extract ${package_name}/${package_name}.tgz
```

## 10. Working with multiple package repositories

Specifying a full package URI might not be practical. It's simpler to specify
a package refspec and let `gpm` find it. But where should it look for it?

When you specify a package using a refspec, `gpm` will have to find the proper
package repository. It will look for this refspec in the repositories listed
in `~/.gpm/sources.list`.

The following command lines will fill `sources.list` with a few (dummy)
package repositories:

```bash
echo "ssh://path.to/my/package-repository.git" >> ~/.gpm/sources.list
echo "ssh://path.to/my/another-repository.git" >> ~/.gpm/sources.list
echo "ssh://path.to/my/yet-another-repository.git" >> ~/.gpm/sources.list
# ...
```

After updating `sources.list`, don't forget to call `gmp update` to update the
cache.

You can then install packages using their refspec.

## 11. Logging

Logs can be enable by setting the `GPM_LOG` environment variable to one of the
following values:

* `trace`
* `debug`
* `info`
* `warn`
* `error`

Example:

```bash
GPM_LOG=info gpm update
```

Logs can be *very* verbose. So it's best to keep only the `gpm` and `gitlfs`
module logs. For example:

```bash
GPM_LOG="gpm=debug,gitlfs=debug" gpm install hello-world/1.0.0
```

## 12. Commands

### 12.1. `update`

Update the cache to feature the latest revision of each repository listed in
`~/.gpm/sources.list`.

Example:

```bash
# first add at least one remote
echo "ssh://github.com/my/awesome-packages.git" >> ~/.gpm/sources.list
echo "ssh://github.com/my/other-packages.git" >> ~/.gpm/sources.list
# ...
# then you can run an update:
gpm update
```

### 12.2. `clean`

Clean the cache. The cache is located in `~/.gpm/cache`.
Cache can be rebuilt using the `update` command.

```bash
gpm clean
```

### 12.3. `install`

Download and install a package.

Example:

```bash
# install the "app" package at version 2.0.0 from repository
# ssh://github.com/my/awesome-packages.git in the /var/www/app folder
gpm install ssh://github.com/my/awesome-packages.git#app/2.0.0 \
    --prefix /var/www/app
```

```bash
# assuming the repository ssh://github.com/my/awesome-packages.git is in
# ~/.gpm/sources.list and the cache has been updated using `gpm update`
gpm install app/2.0.0 --prefix /var/www/app
```

### 12.4. `download`

Download a package in the current working directory.

Example:

```bash
# install the "app" package at version 2.0.0 from repository
# ssh://github.com/my/awesome-packages.git in the /var/www/app folder
gpm download ssh://github.com/my/awesome-packages.git#app/2.0.0 \
    --prefix /var/www/app
```

```bash
# assuming the repository ssh://github.com/my/awesome-packages.git is in
# ~/.gpm/sources.list and the cache has been updated using `gpm update`
gpm download app/2.0.0 --prefix /var/www/app
```

## 13. Integrations

### 13.1. Travis CI

Here is a working Bash script to publish a package from Travis CI:
[script/publish.sh](script/publish.sh).

### 13.2. AppVeyor

Here is a working PowerShell script to publish a package from AppVeyor:
[script/publish.ps1](script/publish.ps1).

### 13.3. GitLab CI

Here is a template to publish a package from GitLab CI:

```yml
.gpm_publish_template: &gpm_publish_template
  stage: publish
  image:
    name: alpine/git:v2.24.1
    entrypoint: ["/bin/sh", "-c"]
  only:
    - tags
  resource_group: ${PACKAGE_REPOSITORY}
  before_script:
    - apk add git-lfs
    - git lfs install --skip-repo
  script:
    - cd ${PACKAGE_ARCHIVE_ROOT} && tar -zcf /tmp/${PACKAGE_NAME}.tar.gz ${PACKAGE_CONTENT} && cd -
    - mkdir -p ~/.ssh && echo -e "Host *\n  StrictHostKeyChecking no\n  IdentityFile /tmp/package-repository-key" > ~/.ssh/config
    - GIT_LFS_SKIP_SMUDGE=1 git clone ${PACKAGE_REPOSITORY} /tmp/package-repository
    - mkdir -p /tmp/package-repository/${PACKAGE_NAME}
    - mv /tmp/${PACKAGE_NAME}.tar.gz /tmp/package-repository/${PACKAGE_NAME}
    - cd /tmp/package-repository/${PACKAGE_NAME}
    - git config --global user.email "${GITLAB_USER_EMAIL}"
    - git config --global user.name "${GITLAB_USER_NAME}"
    - git add ${PACKAGE_NAME}.tar.gz
    - git commit ${PACKAGE_NAME}.tar.gz -m "Publish ${PACKAGE_NAME} version ${PACKAGE_VERSION}."
    - git tag -F "${PACKAGE_CHANGELOG}" "${PACKAGE_NAME}/${PACKAGE_VERSION}"
    - git push
    - git push --tags
```

and an example of how to use this template:

```yml
publish:
  <<: *gpm_publish_template
  variables:
    PACKAGE_VERSION: ${CI_COMMIT_TAG} # the version of the package
    PACKAGE_REPOSITORY: ssh://my.gitlab.com/my-packages-repository.git # the package repository to publish to
    PACKAGE_NAME: ${CI_PROJECT_NAME} # the name of the package
    PACKAGE_ARCHIVE_ROOT: ${CI_PROJECT_DIR}/bin # the folder containing the files to put in the package archive
    PACKAGE_CONTENT: * # the globbing pattern/file list to put in the package archive
    PACKAGE_CHANGELOG: ${CI_PROJECT_DIR}/CHANGELOG.md # the path to the changelog file
```

This template relies on the `PACKAGE_REPOSITORY_KEY` GitLab CI variable.
It must contain a passphrase-less SSH deploy key (with write permissions) to
your package repository.

### 13.4. GitLab Releases

Each package can have its changelog available via the
[GitLab Releases](https://about.gitlab.com/releases/) page of the corresponding
GPM package repository.

The following GitLab CI job templates can be used to create a GitLab release
for a package:

```yml
.gitlab_release_template: &gitlab_release_template
  image: alpine:3.11.5
  only:
    - tags
  dependencies:
    - changelog
  before_script:
    - apk add jq curl
  script: |
    curl --fail --retry 5 \
      -X POST \
      -H 'Content-Type: application/json' \
      -H "PRIVATE-TOKEN: ${RELEASE_AUTH_TOKEN}" \
      https://gitlab.example.com/api/v4/projects/${RELEASE_PROJECT_ID}/releases
      -d "$(
        jq -n \
          --arg name "${RELEASE_NAME}" \
          --arg tag_name "${RELEASE_TAG}" \
          --arg description "$(cat ${RELEASE_NOTES})" \
          {
            name: $name,
            tag_name: $tag_name,
            description: $description
          }
      )"
```

* `RELEASE_AUTH_TOKEN`: the GitLab Personal Access Token used for authentication.
* `RELEASE_PROJECT_ID`: the ID of the GitLab package repository project to create the release on. (ex: `42`)
* `RELEASE_TAG`: the tag of the released GPM package (ex: `my-awesome-package/0.42.3`).
* `RELEASE_NAME`: the name of the released GPM package (ex: `My Awesome Package 0.42.3`).
* `RELEASE_NOTES`: the path to the changelog file of the released GPM package.

Here is an example of a CI job using that template:

```yml
release:
  <<: *gitlab_release_template
  variables:
    RELEASE_AUTH_TOKEN: ${GITLAB_TOKEN}
    RELEASE_TAG: my-awesome-package/${CI_COMMIT_TAG}
    RELEASE_PROJECT_ID: 42
    RELEASE_NAME: My Awesome Package ${CI_COMMIT_TAG}
    RELEASE_NOTES: ${CI_PROJECT_DIR}/CHANGELOG.md
```

### 13.5. GitHub Releases

Each package can have its changelog available via the
[GitHub Releases](https://help.github.com/en/github/administering-a-repository/managing-releases-in-a-repository)
page of the corresponding GPM package repository.

The [gpm-packages release page](https://github.com/aerys/gpm-packages/releases)
is a good example of such integration.

The following `curl` command can be used to create a release for a package:

```yml
curl --fail --retry 5 \
    -X POST \
    -H "Content-Type:application/json" \
    -H "Authorization: token ${RELEASE_AUTH_TOKEN}" \
    https://api.github.com/repos/${RELEASE_PROJECT}/releases \
    -d "$(
        jq -n \
            --arg tag_name "${RELEASE_TAG}" \
            --arg name "${RELEASE_NAME}" \
            --arg body "$(cat ${RELEASE_NOTES})" \
            '{
                tag_name: $tag_name,
                name: $name,
                body: $body,
                draft: false,
                prerelease: false
            }'
    )"
```

* `RELEASE_AUTH_TOKEN`: the GitHub Personal Access Token used for authentication.
* `RELEASE_PROJECT`: the namespaced name of the GitHub package repository to create the release on (ex: `aerys/gpm-packages`).
* `RELEASE_TAG`: the tag of the released GPM package (ex: `my-awesome-package/0.42.3`).
* `RELEASE_NAME`: the name of the released GPM package (ex: `My Awesome Package 0.42.3`).
* `RELEASE_NOTES`: the path to the changelog file of the released GPM package.

Such release creation mechanism can easily be integrated in the CI.

GPM's own GitLab CI configuration features
[a reusable job template](https://github.com/aerys/gpm/blob/0.13.3/.gitlab-ci.yml#L33)
to do exactly that. The result is visible on the
[release page of the gpm-packages repository](https://github.com/aerys/gpm-packages/releases).

## 14. FAQ

### 14.1. Why GPM?

GPM means "Git-based Package Manager".

The main motivation is to have a platform-agnostic package manager, mainly
aimed at distributing binary packages as archives. GPM can be used to
leverage any Git repository as a package repository.

Platforms like GitLab and GitHub are then very handy to manage such package
archives, permissions, etc...

GPM is also available as an all-in-one static binary.
It can be leveraged to download some packages that will be used to bootstrap
a more complex provisioning process.

### 14.2. Why Git? Why not just `curl` or `wget` or whatever?

GPM aims at leveraging the Git ecosystem and features.

Git is great to manage revisions. So it's great at managing package versions!
For example, Git is also used by the Docker registry to store Docker images.

Git also has a safe and secured native authentication/authorization strategy
through SSH. With GitLab, you can safely setup
[deploy keys](https://docs.gitlab.com/ce/ssh/README.html#deploy-keys) to give a
read-only access to your packages.

### 14.3. But Git does not like large binary files!

Yes. Cloning a repository full of large binary files can take a lot of time
and space. You certainly don't want to checkout all the versions of all your
packages everytime you want to install one of them.

That's why you should use [Git LFS](https://git-lfs.github.com/) for your
GPM repositories.

Thanks to [Git LFS](https://git-lfs.github.com/), GPM will download the a
actual binary package only when it is required.

### 14.4. Why storing packages as `*.tar.gz` archives?

Vanilla Git will compress objects. But [Git LFS](https://git-lfs.github.com/)
doesn't store objects in the actual Git repository: they are stored "somewhere
else".

To make sure we don't use too much disk space/bandwidth "somewhere else", the
package archive is stored compressed.

## 15. Contributing

* [Bug tracker](https://github.com/aerys/gpm/issues?q=is%3Aopen+is%3Aissue+label%3Abug)
* [Questions](https://github.com/aerys/gpm/issues?q=is%3Aopen+is%3Aissue+label%3Aquestion)

## 16. License

MIT

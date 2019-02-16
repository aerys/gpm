use std::fmt;
use std::path;

use url::{Url};
use semver::{Version, VersionReq};
use console::style;

#[derive(Debug)]
pub struct PackageVersion {
    raw: String,
    semver_req: Option<VersionReq>,
}

impl PackageVersion {
    pub fn new(s: &String) -> PackageVersion {
        PackageVersion {
            raw: s.to_owned(),
            semver_req: match VersionReq::parse(s.as_str()) {
                Ok(req) => Some(req),
                Err(_) => None,
            },
        }
    }

    pub fn latest() -> PackageVersion {
        PackageVersion {
            raw: String::from("refs/heads/master"),
            semver_req: None,
        }
    }

    pub fn raw(&self) -> &String {
        &self.raw
    }

    pub fn maybe_refspec(&self) -> bool {
        self.semver_req.is_none()
    }

    pub fn is_semver_req(&self) -> bool {
        self.semver_req.is_some()
    }

    pub fn is_latest(&self) -> bool {
        self.raw == "refs/heads/master"
    }

    pub fn matches(&self, version: &str) -> bool {
        if self.is_semver_req() {
            self.semver_req.as_ref().unwrap().matches(&Version::parse(version).unwrap())
        } else {
            self.raw == *version
        }
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", style(&self.raw).magenta())
    }
}

#[derive(Debug)]
pub struct Package {
    remote: Option<String>,
    name: String,
    version: PackageVersion,
}

impl Package {
    pub fn remote(&self) -> &Option<String> {
        return &self.remote;
    }

    pub fn name(&self) -> &String {
        return &self.name;
    }

    pub fn version(&self) -> &PackageVersion {
        return &self.version;
    }

    pub fn parse(s: &String) -> Package {
        let url = s.parse();

        if url.is_ok() {
            let url : Url = url.unwrap();
            let package_and_version = String::from(url.fragment().unwrap());
            let p = Package::parse(&package_and_version);
            let mut remote = url.clone();

            remote.set_fragment(None);

            return Package {
                remote: Some(String::from(remote.as_str())),
                name: p.name,
                version: p.version,
            };

        } else if s.contains("@") {
            let parts : Vec<&str> = s.split("@").collect();

            return Package {
                remote: None,
                name: parts[0].to_string(),
                version: PackageVersion::new(&parts[1].to_string()),
            };
        } else {
            let semver_ops = vec![
                ">=", "<=",
                "=", ">", "<",
                "^", "~",
            ];

            match semver_ops.into_iter().filter(|op| s.contains(op)).last() {
                Some(op) => {
                    let (name, req) = s.split_at(s.find(op).unwrap());

                    Package {
                        remote: None,
                        name: String::from(name),
                        version: PackageVersion::new(&String::from(req)),
                    }
                },
                None => Package {
                    remote: None,
                    name: s.to_owned(),
                    version: PackageVersion::latest(),
                }
            }
        }
    }

    pub fn find_matching_refspec(&self, repo: &git2::Repository) -> Option<String> {
        // First, we attempt to see if there is an exact match.
        // If the version string is set to an actual refspec (ex: "refs/tags/my-package/0.1.0"),
        // this should work.
        if self.version.maybe_refspec() && repo.refname_to_id(self.version.raw()).is_ok() {
            Some(self.version.raw().to_owned())
        } else {
            // Second - and this is the expected normal behavior - we match the version using semver.
            // To do this, we reverse iterate through the repo's tags and find a matching versions.
            // We expect Repository::tag_names to return tag names in chronological order.
            // The last matching tag *must* be the latest one, so the "best matching" one.
            repo.tag_names(None).unwrap().into_iter()
                .filter(|tag_name| -> bool { tag_name.is_some() })
                .map(|tag_name| String::from(tag_name.unwrap()))
                .filter(|tag_name| -> bool {
                    if tag_name.contains("/") {
                        let parts : Vec<&str> = tag_name.split("/").collect();
                        let package_name = parts[0];
                        let package_version = parts[1];

                        if self.name == package_name && self.version().matches(&package_version) {
                            return true
                        }
                    }

                    return false;
                })
                .map(|tag_name| format!("refs/tags/{}", tag_name))
                .last()
        }
    }

    pub fn archive_is_in_repository(&self, repo: &git2::Repository) -> bool {
        let mut path = repo.workdir().unwrap().to_owned();

        path.push(self.get_archive_path(None));

        return path.exists();
    }

    pub fn get_archive_path(&self, rel: Option<path::PathBuf>) -> path::PathBuf {
        match rel {
            Some(rel) => {
                let mut path = path::PathBuf::from(rel);

                path.push(format!("{}/{}", self.name, self.get_archive_filename()));

                path
            },
            None => path::PathBuf::from(format!("{}/{}", self.name, self.get_archive_filename()))
        }
    }

    pub fn get_archive_filename(&self) -> String {
        format!("{}.tar.gz", self.name)
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.version.is_semver_req() {
            write!(f, "{}{}", style(&self.name).cyan(), self.version)
        } else if self.version.is_latest() {
            write!(f, "{}", style(&self.name).cyan())
        } else {
            write!(f, "{}@{}", style(&self.name).cyan(), self.version)
        }
    }
}

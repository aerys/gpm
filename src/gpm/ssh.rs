use std::env;
use std::path;
use std::io;
use std::fs;

extern crate regex;

extern crate console;

extern crate rpassword;

use std::io::prelude::*;

use pest::Parser;

#[derive(Parser)]
#[grammar = "gpm/ssh_config.pest"]
pub struct SSHConfigParser;

pub fn find_ssh_key_for_host(host : &String) -> Result<Option<path::PathBuf>, io::Error> {
    match env::home_dir() {
        Some(path) => {
            let mut path = path::PathBuf::from(path);

            path.push(".ssh");
            path.push("config");

            let mut f = fs::File::open(path.to_owned())?;
            let mut contents = String::new();

            f.read_to_string(&mut contents)?;

            trace!("parsing {:?} to find host {}", path, host);

            let pairs = SSHConfigParser::parse(Rule::config, &contents)
                .unwrap_or_else(|e| panic!("{}", e));

            for pair in pairs {
                let mut inner_pairs = pair.into_inner().flatten();
                let pattern = inner_pairs.find(|p| -> bool {
                    let pattern_str = String::from(p.as_str());

                    match pattern_str.contains("*") {
                        true => {
                            // convert the globbing pattern to a regexp
                            let pattern_str = pattern_str.replace(".", "\\.");
                            let pattern_str = pattern_str.replace("*", ".*");
                            let regexp = regex::Regex::new(pattern_str.as_str())
                                .unwrap();

                            p.as_rule() == Rule::pattern && regexp.is_match(host)
                        },
                        false => p.as_rule() == Rule::pattern && p.as_str() == host
                    }
                });

                match pattern {
                    Some(pattern) => {
                        trace!("found matching host with pattern {:?}", pattern.as_str());

                        let options = inner_pairs.filter(|p| -> bool { p.as_rule() == Rule::option });

                        for option in options {
                            let mut key_and_value = option.into_inner().flatten();
                            let key = key_and_value.find(|p| -> bool { p.as_rule() == Rule::key }).unwrap();
                            let value = key_and_value.find(|p| -> bool { p.as_rule() == Rule::value }).unwrap();

                            if key.as_str() == "IdentityFile" {
                                let path = path::PathBuf::from(value.as_str());

                                trace!("found IdentityFile option with value {:?}", path);
                                return Ok(Some(path));
                            }
                        }
                    },
                    None => continue,
                };
            }

            Ok(None)
        },
        None => Ok(None),
    }
}

pub fn ssh_key_requires_passphrase(buf : &mut io::BufRead) -> bool {
    for line in buf.lines() {
        if line.unwrap().contains("ENCRYPTED") {
            return true;
        }
    }

    return false;
}

pub fn get_ssh_key_and_passphrase(host : &String) -> (Option<path::PathBuf>, Option<String>) {
    let key_path = match find_ssh_key_for_host(host) {
        Ok(path) => path,
        Err(e) => {
            warn!("could not find private key path from ~/.ssh/config: {}", e);

            match env::var("GPM_SSH_KEY") {
                Ok(k) => Some(path::PathBuf::from(k)),
                Err(e) => {
                    warn!("could not read the GPM_SSH_KEY environment variable: {}", e);

                    None
                }
            }
        },
    };

    match key_path {
        Some(key_path) => {
            debug!("authenticate with private key located in {:?}", key_path);

            let mut f = fs::File::open(key_path.to_owned()).unwrap();
            let mut key = String::new();

            f.read_to_string(&mut key).expect("unable to read SSH key from file");
            f.seek(io::SeekFrom::Start(0)).unwrap();

            let mut f = io::BufReader::new(f);

            (
                Some(key_path.to_owned()),
                get_ssh_passphrase(&mut f, format!("Enter passphrase for key {:?}: ", key_path))
            )
        },
        None => {
            warn!("unable to get private key for host {}", &host);

            (None, None)
        }
    }
}

pub fn get_ssh_passphrase(buf : &mut io::BufRead, passphrase_prompt : String) -> Option<String> {
    match ssh_key_requires_passphrase(buf) {
        true => match env::var("GPM_SSH_PASS") {
            Ok(p) => Some(p),
            Err(_) => {
                let mut t = console::Term::stderr();

                trace!("prompt for passphrase");
                let pass_string = rpassword::prompt_password_stderr(passphrase_prompt.as_str())
                    .unwrap();

                t.clear_last_lines(1).unwrap();

                trace!("passphrase fetched from command line");

                Some(pass_string)
            }
        },
        false => None,
    }
}

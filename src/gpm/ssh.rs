use std::env;
use std::path;
use std::io;
use std::fs;
use std::ops::Deref;
use std::io::prelude::*;
use std::io::{Cursor, Read};

use pest::Parser;

extern crate base64;

use base64::{decode};

use zeroize::{Zeroize, Zeroizing};

use crate::gpm::command;

const KEY_MAGIC: &[u8] = b"openssh-key-v1\0";

#[derive(Parser)]
#[grammar = "gpm/ssh_config.pest"]
pub struct SSHConfigParser;

pub fn find_ssh_key_in_ssh_config(host : &String) -> Result<Option<path::PathBuf>, command::CommandError> {
    match dirs::home_dir() {
        Some(home_path) => {
            let mut ssh_config_path = path::PathBuf::from(home_path);

            ssh_config_path.push(".ssh");
            ssh_config_path.push("config");

            let mut f = fs::File::open(ssh_config_path.to_owned())?;
            let mut contents = String::new();

            f.read_to_string(&mut contents)?;

            trace!("parsing {:?} to find host {}", ssh_config_path, host);

            let pairs = SSHConfigParser::parse(Rule::config, &contents)?;

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

pub fn find_default_ssh_key() -> Option<path::PathBuf> {
    match dirs::home_dir() {
        Some(home_path) => {
            let mut id_rsa_path = path::PathBuf::from(home_path);

            id_rsa_path.push(".ssh");
            id_rsa_path.push("id_rsa");

            if id_rsa_path.exists() && id_rsa_path.is_file() {
                Some(id_rsa_path)
            } else {
                None
            }
        },
        None => None
    }
}

pub fn find_ssh_key_for_host(host : &String) -> Option<path::PathBuf> {
    match find_ssh_key_in_ssh_config(host) {
        Ok(path) => match path {
            Some(_) => path,
            None => find_default_ssh_key(),
        },
        Err(e) => {
            warn!("Unable to get SSH key from ~/.ssh/config: {}", e);

            find_default_ssh_key()
        },
    }
}

fn read_utf8(c: &mut Cursor::<&[u8]>) -> io::Result<String> {
    let mut buf = read_string(c)?;
    // Make data be zeroed even if an error occurred
    // So we cannot directly use `String::from_utf8()`
    match std::str::from_utf8(&buf) {
        Ok(_) => unsafe {
            // We have checked the string using `str::from_utf8()`
            // To avoid memory copy, just use `from_utf8_unchecked()`
            Ok(String::from_utf8_unchecked(buf))
        },
        Err(_) => {
            buf.zeroize();
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid UTF-8 sequence",
            ))
        }
    }
}

fn read_string(c: &mut Cursor::<&[u8]>) -> io::Result<Vec<u8>> {
    let len = read_uint32(c)? as usize;
    let mut buf = vec![0u8; len];
    match c.read_exact(buf.as_mut_slice()) {
        Ok(_) => Ok(buf),
        Err(e) => {
            buf.zeroize();
            Err(e)
        }
    }
}

fn read_uint32(c: &mut Cursor::<&[u8]>) -> io::Result<u32> {
    let mut buf = Zeroizing::new([0u8; 4]);
    c.read_exact(&mut *buf)?;
    Ok(u32::from_be_bytes(*buf))
}

pub fn ssh_key_requires_passphrase(
    buf: &mut dyn io::BufRead
) -> io::Result<bool> {
    debug!("attempting to detect SSH private key encryption (OpenSSH <= 6.4)");
    let metadata_regex = regex::Regex::new(r"(.*): (.*)")
        .unwrap();
    let (metadata, content) : (Vec<String>, Vec<String>) = buf.lines()
        // Remove comments
        .filter(|line| !line.as_ref().unwrap().starts_with('-'))
        .collect::<io::Result<Vec<String>>>()?
        .iter()
        .map(String::clone)
        .partition(|line| metadata_regex.is_match(line));

    if metadata.iter().any(|l| l.contains("ENCRYPTED")) {
        debug!("found ENCRYPTED keyword");
        return Ok(true);
    }

    debug!("attempting to decode SSH private key (OpenSSH >= 6.5)");
    // The following code is loosely inspired from the rust-osshkeys crate
    // (https://crates.io/crates/osshkeys) to make a basic check of the SSH
    // private key header and read the cipher name:
    //
    // https://github.com/Leo1003/rust-osshkeys/blob/ed38963db967239de05af3473fe4000917a2c2c8/src/format/ossh_privkey.rs#L23
    //
    // The read_uint32(), read_string() and read_utf8() above also come from
    // the same project:
    //
    // https://github.com/Leo1003/rust-osshkeys/blob/ed38963db967239de05af3473fe4000917a2c2c8/src/sshbuf.rs#L167
    if let Ok(keydata) = decode(content.concat()) {
        if keydata.len() >= 16 && &keydata[0..15] == KEY_MAGIC {
            let mut reader = Cursor::new(keydata.deref());
            reader.set_position(15);
    
            let ciphername = read_utf8(&mut reader)?;

            debug!("found cipher {}", ciphername);

            return Ok(ciphername != "none");
        }
    }

    return Ok(false);
}

pub fn get_ssh_key_and_passphrase(host : &String) -> (Option<path::PathBuf>, Option<String>) {

    let key = match env::var("GPM_SSH_KEY") {
        Ok(k) => {
            let path = path::PathBuf::from(k);

            if path.exists() && path.is_file() {
                Some(path)
            } else {
                warn!(
                    "Ignoring the GPM_SSH_KEY environment variable: {:?} does not exist or is not a file.",
                    path
                );

                find_ssh_key_for_host(host)
            }
        },
        Err(e) => {
            warn!("could not read the GPM_SSH_KEY environment variable: {}", e);

            find_ssh_key_for_host(host)
        }
    };

    match key {
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

pub fn get_ssh_passphrase(buf : &mut dyn io::BufRead, passphrase_prompt : String) -> Option<String> {
    match ssh_key_requires_passphrase(buf) {
        Ok(true) => match env::var("GPM_SSH_PASS") {
            Ok(p) => Some(p),
            Err(_) => {
                trace!("prompt for passphrase");
                let pass_string = rpassword::prompt_password_stderr(passphrase_prompt.as_str())
                    .unwrap();

                trace!("passphrase fetched from command line");

                Some(pass_string)
            }
        },
        Ok(false) => None,
        Err(e) => {
            error!("Unable to read SSH private key: {}", e);

            None
        },
    }
}

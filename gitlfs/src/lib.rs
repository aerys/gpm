#![deny(warnings)]

#[macro_use]
extern crate log;

#[macro_use]
extern crate json;

extern crate reqwest;

extern crate ssh2;

extern crate url;

pub mod lfs {
    use json;

    use ssh2::Session;

    use url::{Url};
    
    use reqwest;
    use reqwest::header::{Accept, ContentType, Authorization, qitem};

    use std::io::prelude::*;
    use std::net::{TcpStream};
    use std::str;
    use std::path;
    use std::io;
    use std::fs;

    pub fn parse_lfs_link_file(p : &path::Path) -> Result<Option<(String, String)>, io::Error> {
        debug!("attempting to match {} as an LFS link", p.to_str().unwrap());

        let f = fs::File::open(p)?;
        let mut f = io::BufReader::new(f);
        let mut buf = String::new();

        let is_lfs_link = match f.read_line(&mut buf) {
            Ok(_) => buf == "version https://git-lfs.github.com/spec/v1\n",
            Err(e) => return Err(e),
        };

        if is_lfs_link {
            debug!("file is an LFS link, reading LFS data");

            let mut oid_line = String::new();
            let mut size_line = String::new();
            
            f.read_line(&mut oid_line).expect("unable to read oid from LFS link");
            f.read_line(&mut size_line).expect("unable to read size from LFS link");

            // skip "oid sha256:"
            let oid = oid_line[11 .. oid_line.len() - 1].to_string();
            // skip "size "
            let size = size_line[5 .. size_line.len() - 1].to_string();

            debug!("oid = {}, size = {}", oid, size);

            Ok(Some((oid, size)))
        } else {
            debug!("file is not an LFS link");
            Ok(None)
        }
    }

    pub fn get_lfs_download_link(
        oid : &String,
        size : &String,
        refspec : Option<String>,
        url : String,
        auth_token : Option<String>,
    ) -> Result<(Option<String>, String), reqwest::Error> {
        // https://github.com/git-lfs/git-lfs/blob/master/docs/api/batch.md
        let mut payload = object!{
            "operation" => "download",
            "transfers" => array!["basic"],
            "objects" => array![
                object!{
                    "oid" => oid.to_owned(),
                    "size" => size.to_owned().parse::<u32>().unwrap(),
                }
            ]
        };

        if refspec.is_some() {
            payload["ref"] = object!{
                "name" => refspec.unwrap(),
            };
        }

        let client = reqwest::Client::new();
        let url : Url = format!("{}/objects/batch", url).parse().unwrap();
        let mut req = client.post(url.to_owned());

        req.body(payload.to_string())
            .header(Accept(vec![qitem("application/vnd.git-lfs+json".parse().unwrap())]))
            .header(ContentType("application/vnd.git-lfs+json".parse().unwrap()));

        if auth_token.is_some() {
            req.header(Authorization(auth_token.unwrap().to_owned()));
        }

        trace!("sending LFS object batch payload to {}:\n{}", &url, payload.pretty(2));

        let mut res = req.send()?;

        if !res.status().is_success() {
            warn!("LFS server responded with error code: {}", res.status());
        }

        let data = json::parse(res.text().unwrap().as_str())
            .expect("failed at parsing LFS server response");

        if !res.status().is_success() {
            warn!("error message: {}", res.text().unwrap());
        }

        trace!("response from LFS server:\n{}", data.pretty(2));

        let auth_token = match data["objects"][0]["actions"]["download"]["header"]["Authorization"].as_str() {
            Some(s) => Some(String::from(s)),
            None => None,
        };
        let url = String::from(data["objects"][0]["actions"]["download"]["href"].as_str().unwrap());

        Ok((auth_token, url))
    }

    pub fn resolve_lfs_link(
        repository : Url,
        refspec : Option<String>,
        p : &path::Path, 
        target : Option<&path::Path>,
        private_key : Option<String>) -> Result<bool, io::Error>
    {
        let (oid, size) = match parse_lfs_link_file(p)? {
            Some((o, s)) => (o, s),
            None => return Ok(false),
        };
        let (auth_token, url) = match get_lfs_auth_token(repository, "download", private_key) {
            Ok((t, u)) => (t, u),
            Err(e) => panic!("unable to get LFS batch authorization token: {}", e),
        };
        let (auth_token, url) = match get_lfs_download_link(&oid, &size, refspec, url, auth_token) {
            Ok((h, u)) => (h, u),
            Err(e) => panic!("unable to fetch LFS download link: {}", e),
        };

        match download_lfs_object(target.unwrap_or(p), auth_token, &url) {
            Ok(()) => Ok(true),
            Err(e) => panic!("failed to donwload LFS object: {}", e),
        }
    }

    pub fn guess_lfs_url(repository : Url) -> String {
        format!("https://{}{}/info/lfs", repository.host_str().unwrap(), repository.path())
    }

    // https://github.com/git-lfs/git-lfs/blob/master/docs/api/authentication.md
    pub fn get_lfs_auth_token(
        repository : Url,
        op : &str,
        private_key : Option<String>) -> Result<(Option<String>, String), json::Error>
    {
        let host_and_port = format!(
            "{}:{}",
            repository.host_str().unwrap(),
            repository.port().unwrap_or(22)
        );

        match private_key {
            None => {
                debug!("no SSH private key provided: continue without authentication");

                return Ok((None, guess_lfs_url(repository)));
            },
            Some(ssh_key) => {
                debug!("attempting to fetch Git LFS auth token from {}", host_and_port);
                debug!("connecting to {}", host_and_port);

                let tcp = TcpStream::connect(host_and_port).unwrap();
                let mut sess = Session::new().unwrap();
                
                debug!("SSH session handshake");
                sess.handshake(&tcp).unwrap();

                debug!("attempt SSH public key authentication with key {}", ssh_key);

                let ssh_key_path = path::Path::new(&ssh_key);
                
                match sess.userauth_pubkey_file("git", None, ssh_key_path, None) {
                    Ok(()) => {
                        debug!("SSH session authenticated");

                        let path = &repository.path()[1..];
                        let command = format!("git-lfs-authenticate {} {}", path, op);
                        let mut channel = sess.channel_session().unwrap();
                        
                        debug!("execute \"{}\" command over SSH", command);
                        channel.exec(&command).expect("error while running the git-lfs-authenticate command via SSH");

                        let mut s = String::new();
                        channel.read_to_string(&mut s).unwrap();
                        channel.wait_close().expect("error while waiting for SSH channel to close");

                        let json = json::parse(&s)?;

                        return Ok((
                            Some(String::from(json["header"]["Authorization"].as_str().unwrap())),
                            String::from(json["href"].as_str().unwrap()),
                        ));                    },
                    Err(e) => {
                        warn!("failed to authenticate with SSH key: {}", e);
                        warn!("continue without authentication");

                        return Ok((None, guess_lfs_url(repository)));
                    }
                };
            },
        };
    }

    pub fn download_lfs_object(
        path : &path::Path,
        auth_token : Option<String>,
        url : &String,
    ) -> Result<(), reqwest::Error> {
        debug!("preparing to download LFS object in {}", path.to_str().unwrap());

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .expect("unable to open LFS object target file");

        debug!("start downloading LFS object into {}", path.to_str().unwrap());

        let client = reqwest::Client::new();
        let mut req = client.get(url);

        if auth_token.is_some() {
            req.header(Authorization(auth_token.unwrap()));
        }

        let mut res = req.send()?;


        match io::copy(&mut res, &mut file) {
            Ok(_) => {
                debug!("LFS object download complete");
                
                Ok(())
            },
            Err(e) => panic!("failed to write LFS object: {}", e),
        }
    }
}

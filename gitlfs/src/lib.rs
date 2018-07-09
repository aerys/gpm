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
    use std::env;

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
        url : &String,
        auth_token : &String,
    ) -> Result<(String, String), reqwest::Error> {
        // https://github.com/git-lfs/git-lfs/blob/master/docs/api/batch.md
        let payload = object!{
            "operation" => "download",
            "transfers" => array!["basic"],
            "ref" => object!{
                "name" => "refs/heads/3.0",
            },
            "objects" => array![
                object!{
                    "oid" => oid.to_owned(),
                    "size" => size.to_owned(),
                }
            ]
        };

        let client = reqwest::Client::new();
        let url : Url = format!("{}objects/batch", url).parse().unwrap();
        let mut res = client.post(url)
            .body(payload.to_string())
            .header(Accept(vec![qitem("application/vnd.git-lfs+json".parse().unwrap())]))
            .header(ContentType("application/vnd.git-lfs+json".parse().unwrap()))
            .header(Authorization(auth_token.to_owned()))
            .send()?;

        let data = json::parse(res.text().unwrap().as_str())
            .expect("failed at parsing LFS server response");

        let header = String::from(data["objects"][0]["actions"]["download"]["header"]["Authorization"].as_str().unwrap());
        let url = String::from(data["objects"][0]["actions"]["download"]["href"].as_str().unwrap());

        Ok((header, url))
    }

    pub fn resolve_lfs_link(repository : Url, p : &path::Path, target : Option<&path::Path>) -> Result<bool, io::Error> {
        let (oid, size) = match parse_lfs_link_file(p)? {
            Some((o, s)) => (o, s),
            None => return Ok(false),
        };
        let (auth_token, url) = match get_lfs_auth_token(repository, "download") {
            Ok((t, u)) => (t, u),
            Err(e) => panic!("unable to get LFS batch authorization token: {}", e),
        };
        let (auth_token, url) = match get_lfs_download_link(&oid, &size, &url, &auth_token) {
            Ok((h, u)) => (h, u),
            Err(e) => panic!("unable to fetch LFS download link: {}", e),
        };

        match download_lfs_object(target.unwrap_or(p), &auth_token, &url) {
            Ok(()) => Ok(true),
            Err(e) => panic!("failed to donwload LFS object: {}", e),
        }
    }

    // https://github.com/git-lfs/git-lfs/blob/master/docs/api/authentication.md
    pub fn get_lfs_auth_token(repository : Url, op : &str) -> Result<(String, String), json::Error> {
        let host_and_port = format!(
            "{}:{}",
            repository.host_str().unwrap(),
            repository.port().unwrap_or(22)
        );

        debug!("preparing to fetch Git LFS auth token from {}", host_and_port);
        debug!("connecting to {}", host_and_port);
        let tcp = TcpStream::connect(host_and_port).unwrap();
        let mut sess = Session::new().unwrap();
        
        debug!("SSH session handshake");
        sess.handshake(&tcp).unwrap();
        
        let pubkey = match env::var("GPM_SSH_KEY") {
            Ok(p) => p,
            Err(e) => panic!("could not retrieve SSH key: {}", e)
        };
        let pubkey_path = path::Path::new(&pubkey);

        debug!("SSH public key authentication with key {}", pubkey);
        match sess.userauth_pubkey_file("git", None, pubkey_path, None) {
            Ok(()) => debug!("SSH session authenticated"),
            Err(e) => panic!("failed to authenticate: {}", e)
        };

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
            String::from(json["header"]["Authorization"].as_str().unwrap()),
            String::from(json["href"].as_str().unwrap()),
        ));
    }

    pub fn download_lfs_object(
        path : &path::Path,
        auth_token : &String,
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
        let mut res = client.get(url)
            .header(Authorization(auth_token.to_owned()))
            .send()?;


        match io::copy(&mut res, &mut file) {
            Ok(_) => {
                debug!("LFS object download complete");
                
                Ok(())
            },
            Err(e) => panic!("failed to write LFS object: {}", e),
        }
    }
}

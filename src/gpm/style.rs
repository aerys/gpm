
use console::style;

use url::Url;

pub fn command(c : &String) -> String {
    format!("{}", style(c).green())
}

pub fn remote_url(remote : &String) -> String {
    let mut remote = remote.clone();
    let url : Url = remote.parse().unwrap();
    let path = url.path();
    let l = remote.len();

    remote.truncate(l - path.len());

    format!("{}{}", style(remote).dim(), path)
}

pub fn package_name(name : &String) -> String {
    format!("{}", style(name).cyan())
}

pub fn package_extension(ext : &String) -> String {
    format!("{}", style(ext).dim())
}

pub fn refspec(r : &String) -> String {
    format!("{}", style(&r).magenta())
}

pub fn revision(r : &String) -> String {
    format!("{}", style(&r).magenta())
}

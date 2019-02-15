use url::{Url};

pub fn parse_ref(package_ref : &String) -> (Option<String>, String, String) {
    let url = package_ref.parse();

    if url.is_ok() {
        let url : Url = url.unwrap();
        let package_and_version = String::from(url.fragment().unwrap());
        let (_, package, version) = parse_ref(&package_and_version);
        let mut remote = url.clone();

        remote.set_fragment(None);

        return (
            Some(String::from(remote.as_str())),
            package,
            version,
        );

    } else {
        if package_ref.contains("=") {
            let parts : Vec<&str> = package_ref.split("=").collect();

            return (
                None,
                parts[0].to_string(),
                parts[1].to_string(),
            );
        }

        if package_ref.contains("/") {
            let parts : Vec<&str> = package_ref.split("/").collect();

            return (
                None,
                parts[0].to_string(),
                package_ref.to_owned(),
            );
        }

        (None, package_ref.to_owned(), String::from("refs/heads/master"))
    }
}

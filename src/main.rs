#[macro_use]
extern crate clap; 
use clap::{App, Arg};

#[macro_use]
extern crate log;

#[macro_use]
extern crate pest_derive;

use dotenv::dotenv;

mod gpm;

fn main() {
    dotenv().ok();

    pretty_env_logger::init_custom_env("GPM_LOG");

    let matches = App::new("gpm")
        .about("Git-based package manager.")
        .version(crate_version!())
        .setting(clap::AppSettings::ArgRequiredElseHelp)
        .subcommand(clap::SubCommand::with_name("install")
            .about("Install a package")
            .arg(Arg::with_name("package"))
            .arg(Arg::with_name("prefix")
                .help("The prefix to the package install path")
                .default_value("/")
                .long("--prefix")
                .required(false)
            )
            .arg(Arg::with_name("force")
                .help("Replace existing files")
                .long("--force")
                .takes_value(false)
                .required(false)
            )
        )
        .subcommand(clap::SubCommand::with_name("download")
            .about("Download a package")
            .arg(Arg::with_name("package"))
            .arg(Arg::with_name("force")
                .help("Replace existing files")
                .long("--force")
                .takes_value(false)
                .required(false)
            )
        )
        .subcommand(clap::SubCommand::with_name("update")
            .about("Update all package repositories")
        )
        .subcommand(clap::SubCommand::with_name("clean")
            .about("Clean all repositories from cache")
        )
        .get_matches();

    for command in gpm::command::commands().iter() {
        match command.matched_args(&matches) {
            Some(command_args) => {
                match (*command).run(command_args) {
                    Ok(_) => {
                        // nothing
                    },
                    Err(e) => {
                        error!("command failed with error {:?}", e);
                        std::process::exit(1);
                    }
                };
                break;
            },
            None => continue,
        };
    }

    std::process::exit(0);
}


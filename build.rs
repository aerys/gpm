
extern crate vergen;
extern crate anyhow;

use vergen::{Config, vergen};
use anyhow::Result;

fn main() -> Result<()> {
  // Generate the default 'cargo:' instruction output
  vergen(Config::default())
}

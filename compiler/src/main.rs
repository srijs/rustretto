extern crate classfile;
extern crate jar;
#[macro_use]
extern crate failure;
extern crate petgraph;
#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate structopt;
extern crate tempdir;

use std::env;
use std::path::PathBuf;

use failure::Fallible;
use structopt::StructOpt;

mod blocks;
mod classes;
mod compile;
mod disasm;
mod driver;
mod frame;
mod generate;
mod loader;
mod translate;
mod types;
mod utils;

use driver::Driver;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "compile",
    about = "Compile JVM classfiles into a native executable."
)]
struct Compile {
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

fn compile(c: Compile) -> Fallible<()> {
    let home = PathBuf::from(
        env::var("JAVA_HOME").map_err(|_| format_err!("could not read JAVA_HOME variable"))?,
    );

    let driver = Driver::new(home)?;

    driver.compile(&c.input)?;
    driver.link()?;

    Ok(())
}

fn main() {
    env_logger::init();
    if let Err(err) = compile(Compile::from_args()) {
        println!("Error: {:?}", err);
    }
}

use std::alloc::System;
use std::env;
use std::path::PathBuf;

use failure::{format_err, Fallible};
use structopt::StructOpt;

mod blocks;
mod classes;
mod compile;
mod disasm;
mod driver;
mod frame;
mod generate;
mod loader;
mod mangle;
mod target;
mod translate;
mod types;
mod vtable;

use crate::driver::Driver;

#[global_allocator]
static GLOBAL: System = System;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "compile",
    about = "Compile JVM classfiles into a native executable."
)]
struct Compile {
    #[structopt(parse(from_os_str), short = "o")]
    output: PathBuf,
    #[structopt(parse(from_os_str), short = "r")]
    runtime: PathBuf,
    #[structopt(parse(from_os_str))]
    inputs: Vec<PathBuf>,
    #[structopt(long = "main")]
    main: String,
    #[structopt(short = "O")]
    optimize: bool,
    #[structopt(parse(from_os_str), long = "save-temp")]
    save_temp: Option<PathBuf>,
}

fn compile(c: Compile) -> Fallible<()> {
    let home = PathBuf::from(
        env::var("JAVA_HOME").map_err(|_| format_err!("could not read JAVA_HOME variable"))?,
    );

    let platform = platforms::guess_current()
        .ok_or_else(|| format_err!("could not determine host platform"))?;

    let mut driver = Driver::new(home, platform.clone(), c.optimize)?;

    driver.compile(&c.main, &c.inputs)?;

    if let Some(ref temppath) = c.save_temp {
        driver.dump(temppath)?;
    }

    driver.link(&c.runtime, &c.output)?;

    Ok(())
}

fn main() {
    env_logger::init();
    if let Err(err) = compile(Compile::from_args()) {
        println!("Error: {}", err);
        println!("{}", err.backtrace());
        std::process::exit(1);
    }
}

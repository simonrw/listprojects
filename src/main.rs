use skim::prelude::*;
use std::sync::Arc;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opts {}

fn main() {
    let _args = Opts::from_args();
    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(false)
        .build()
        .unwrap();
}

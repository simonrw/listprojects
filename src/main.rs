use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Opts {}

fn main() {
    let opts = Opts::from_args();
    dbg!(opts);
    println!("Hello, world!");
}

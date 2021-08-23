use skim::prelude::*;
use std::sync::Arc;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opts {}

fn worker(tx: crossbeam_channel::Sender<Arc<dyn SkimItem + 'static>>) {
    std::thread::sleep(std::time::Duration::from_secs(1));
    tx.send(Arc::new("test".to_string())).unwrap();
}

fn main() {
    let _args = Opts::from_args();
    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(false)
        .build()
        .unwrap();

    let (tx, rx) = crossbeam_channel::bounded(100);

    let thread_tx = tx.clone();
    let handle = std::thread::spawn(move || worker(thread_tx));

    let selected_items = Skim::run_with(&options, Some(rx))
        .map(|out| out.selected_items)
        .unwrap_or_else(|| Vec::new());

    for item in selected_items {
        println!("{}", item.output());
    }

    handle.join().unwrap();
}

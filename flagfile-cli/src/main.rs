use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "Flagfile")]
#[command(version = "1.0")]
#[command(about = "Feature flagging for developers", long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Command
}

#[derive(Subcommand, Debug)]
enum Command {
    Init, // creates empty file with demo flag
    List, // lists all flags in flagfile
    Validate, // parses and validates all rules
}

fn main() {
    let cli = Args::parse();
    dbg!(cli.cmd);
    println!("Hello, world from Flagfile cli");
}

use clap::Parser;

#[derive(Parser)]
struct Args {
    input: std::path::PathBuf,
}

fn main() {
    let args = Args::parse();
    if let Some(path) = args.input.file_name() {
        println!("Started with {}", path.display());
        return;
    }
    println!("Started with invalid path")
}

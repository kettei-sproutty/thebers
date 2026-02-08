//! Thebe CLI — build tool for `.trs` single-file components.

mod discover;
mod emit;

use std::path::PathBuf;

use anyhow::Context as _;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
#[command(name = "thebe", about = "Build tool for Thebe single-file components")]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Subcommand)]
enum Command {
  /// Compile `.trs` route files into generated Rust modules.
  Build {
    /// Directory containing `.trs` route files.
    #[arg(long, default_value = "src/routes")]
    routes: PathBuf,

    /// Directory containing reusable `.trs` components.
    #[arg(long, default_value = "src/components")]
    components: PathBuf,

    /// Output directory for generated code.
    #[arg(long, default_value = ".thebe")]
    output: PathBuf,

    /// Allow a non-dot-prefixed output directory (e.g. `generated`).
    #[arg(long, default_value_t = false)]
    force: bool,
  },
}

fn main() -> Result<()> {
  let cli = Cli::parse();

  match cli.command {
    Command::Build {
      routes,
      components,
      output,
      force,
    } => {
      let cwd = std::env::current_dir()?;
      let routes_dir = cwd.join(&routes);
      let output_dir = cwd.join(&output);
      let components_dir = cwd.join(&components);

      anyhow::ensure!(
        routes_dir.exists(),
        "routes directory not found: {}",
        routes_dir.display()
      );

      let comp_dir = if components_dir.exists() {
        Some(components_dir.as_path())
      } else {
        None
      };

      let entries = discover::discover_routes(&routes_dir, comp_dir)
        .context("discovering route files")?;

      if entries.is_empty() {
        println!("No .trs files found in {}", routes_dir.display());
        return Ok(());
      }

      println!("Found {} route(s):", entries.len());
      for entry in &entries {
        println!(
          "  {} → {}",
          entry.relative_path.display(),
          entry.url_path
        );
      }

      emit::emit_all(&entries, &output_dir, force)
        .context("emitting generated code")?;

      println!(
        "\nGenerated code written to {}",
        output_dir.display()
      );
      Ok(())
    }
  }
}

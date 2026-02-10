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

    /// Directory containing static assets served at `/public/…`.
    #[arg(long, default_value = "src/public")]
    public: PathBuf,

    /// Output directory for generated code.
    #[arg(long, default_value = ".thebe")]
    output: PathBuf,

    /// Allow a non-dot-prefixed output directory (e.g. `generated`).
    #[arg(long, default_value_t = false)]
    force: bool,
  },

  /// Parse and validate `.trs` files without generating code.
  Check {
    /// Directory containing `.trs` route files.
    #[arg(long, default_value = "src/routes")]
    routes: PathBuf,

    /// Directory containing reusable `.trs` components.
    #[arg(long, default_value = "src/components")]
    components: PathBuf,
  },
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
  let cli = Cli::parse();

  match cli.command {
    Command::Build {
      routes,
      components,
      public,
      output,
      force,
    } => {
      let cwd = std::env::current_dir()?;
      let routes_dir = cwd.join(&routes);
      let output_dir = cwd.join(&output);
      let components_dir = cwd.join(&components);
      let public_dir = cwd.join(&public);

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

      let public_arg = if public_dir.exists() {
        Some(public.as_path())
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

      emit::emit_all(&entries, &output_dir, force, public_arg)
        .context("emitting generated code")?;

      println!(
        "\nGenerated code written to {}",
        output_dir.display()
      );
      Ok(())
    }
    Command::Check { routes, components } => {
      let cwd = std::env::current_dir()?;
      let routes_dir = cwd.join(&routes);
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

      let mut errors = 0u32;
      let mut warnings_count = 0u32;

      for entry in &entries {
        let source = std::fs::read_to_string(&entry.source_path)
          .with_context(|| format!("reading {}", entry.source_path.display()))?;

        let fname = entry.relative_path.display().to_string();

        // Parse
        let ast = match thebe_ast::parse(&source) {
          Ok(ast) => ast,
          Err(e) => {
            let _ = thebe_ast::diagnostics::eprint_error(&e, &source, Some(&fname));
            errors += 1;
            continue;
          }
        };

        // Lower
        let ir = match thebe_compiler::lower(&source, &ast) {
          Ok(ir) => ir,
          Err(e) => {
            let _ = thebe_compiler::diagnostics::eprint_error(&e, &source, Some(&fname));
            errors += 1;
            continue;
          }
        };

        // Validate
        let warnings = thebe_compiler::validate(&ir);
        if !warnings.is_empty() {
          for w in &warnings {
            let _ = thebe_compiler::diagnostics::eprint_warning(w, &source, Some(&fname));
          }
          #[allow(clippy::cast_possible_truncation)]
          {
            warnings_count += warnings.len().min(u32::MAX as usize) as u32;
          }
        }
      }

      // Summary
      println!(
        "\nChecked {} file(s): {} error(s), {} warning(s)",
        entries.len(),
        errors,
        warnings_count
      );

      if errors > 0 {
        std::process::exit(1);
      }

      Ok(())
    }
  }
}

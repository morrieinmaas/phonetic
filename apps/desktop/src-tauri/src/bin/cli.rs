//! `phonetic-cli` — headless measurement.

use clap::{Parser, Subcommand, ValueEnum};
use phonetic_lib::cli::{run_devices, run_measure, run_selftest, OutputFormat};

#[derive(Parser)]
#[command(name = "phonetic-cli", about = "Headless audio measurement", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Measure a transfer function from a multi-channel WAV file
    Measure {
        /// Path to the WAV file
        #[arg(long)]
        file: String,
        /// Channel carrying the reference signal
        #[arg(long, default_value_t = 0)]
        reference_channel: usize,
        /// Channel carrying the measurement microphone
        #[arg(long, default_value_t = 1)]
        measurement_channel: usize,
        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// List available audio input devices
    Devices,
    /// Verify the DSP against a known signal
    Selftest,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Json,
    Csv,
}

impl From<Format> for OutputFormat {
    fn from(f: Format) -> Self {
        match f {
            Format::Json => OutputFormat::Json,
            Format::Csv => OutputFormat::Csv,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Measure {
            file,
            reference_channel,
            measurement_channel,
            format,
        } => run_measure(&file, reference_channel, measurement_channel, format.into()),
        Command::Devices => run_devices(),
        Command::Selftest => run_selftest(),
    };

    match result {
        Ok(output) => println!("{output}"),
        Err(message) => {
            eprintln!("error: {message}");
            std::process::exit(1);
        }
    }
}

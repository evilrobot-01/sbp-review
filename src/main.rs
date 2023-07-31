use crate::clippy::Message;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::{fs, process::Command};
use terminal_link::Link;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyses code for known issues.
    Code,
    /// Analyses manifest(s) for known issues.
    Manifests,
    /// Executes available tests.
    Tests,
    /// Executes available benchmarks as tests.
    Benchmarks,
}

fn main() {
    match &Cli::parse().command {
        None => {}
        Some(Commands::Code) => lint(),
        Some(Commands::Manifests) => metadata(),
        Some(Commands::Tests) => test(),
        Some(Commands::Benchmarks) => benchmark(),
    }
}

fn lint() {
    println!("Analysing code via clippy...");

    const CLIPPY_CONFIG: &str = "clippy.toml";
    let clippy_config_exists = std::fs::metadata(CLIPPY_CONFIG).is_ok();
    if !clippy_config_exists {
        const CONFIG: &str = "too-many-lines-threshold=30";
        std::fs::write(CLIPPY_CONFIG, CONFIG).unwrap();
    }

    let args = [
        // warn
        "-Wclippy::too-many-lines",
        // deny
        &format!("-D{}", clippy::EXPECT_UNUSED),
        "-Dclippy::unwrap_used",
        "-Dclippy::ok_expect",
        "-Dclippy::integer_division",
        "-Dclippy::indexing_slicing",
        "-Dclippy::integer_arithmetic",
        "-Dclippy::match_on_vec_items",
        "-Dclippy::manual_strip",
        "-Dclippy::await_holding_refcell_ref",
    ];

    let output = Command::new("cargo")
        .arg("clippy")
        .arg("--message-format=json")
        .arg("--")
        .args(args)
        .output()
        .unwrap();

    if output.stderr.len() > 0 {
        println!("{}", String::from_utf8_lossy(&output.stderr))
    }

    let mut matches = Vec::new();
    let output = String::from_utf8_lossy(&output.stdout);
    for line in output.lines() {
        match serde_json::from_str::<clippy::Match>(line) {
            Ok(m) => matches.push(m),
            Err(e) => {
                println!("{} {}", e, line)
            }
        }
    }

    if !clippy_config_exists {
        fs::remove_file(CLIPPY_CONFIG).unwrap();
    }

    // Output results
    for message in matches
        .iter()
        .filter_map(|m| m.message.as_ref())
        .filter(|m| !ignored(m))
    {
        // todo: sort by file path, line number
        print!(
            "{} {} {}",
            match message.level.as_str() {
                "warning" => message.level.yellow(),
                "error" => message.level.red(),
                _ => message.level.normal(),
            },
            message.code.as_ref().map_or("".into(), |c| c.code.as_str()),
            message.message,
        );
        // add help
        for item in message
            .children
            .iter()
            .filter(|m| m.level == "help" && !m.message.starts_with("for further information"))
        {
            print!(" {} {}", "help:".bold(), item.message)
        }
        match message.spans.get(0) {
            None => {}
            Some(span) => {
                let text = format!(
                    "./{}:{}:{}",
                    span.file_name, span.line_start, span.column_start
                );
                let url = format!(
                    "file:///{}/{}:{}:{}",
                    std::env::current_dir()
                        .unwrap()
                        .into_os_string()
                        .into_string()
                        .unwrap(),
                    span.file_name,
                    span.line_start,
                    span.column_start
                );
                println!(" at {}", Link::new(&text, &url).to_string().cyan())
            }
        }
    }
}

fn ignored(message: &Message) -> bool {
    const EXPECT_USED_IGNORED: [&str; 3] = [
        "#[pallet::error]",
        "#[pallet::pallet]",
        "#[pallet::storage]",
    ];
    message.code.as_ref().map(|c| c.code.as_str()) == Some(clippy::EXPECT_UNUSED)
        && message.spans.iter().any(|s| {
            s.text
                .iter()
                .any(|t| EXPECT_USED_IGNORED.iter().any(|i| t.text.contains(i)))
        })
}

fn metadata() {
    println!("Analysing manifest(s) via metadata...");

    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .output()
        .unwrap();

    let output = String::from_utf8_lossy(&output.stdout);
    match serde_json::from_str::<manifests::Metadata>(&output) {
        Ok(metadata) => {
            for package in metadata.packages {
                println!(
                    "{}",
                    Link::new(&package.name, &format!("file:///{}", package.manifest_path))
                        .to_string()
                        .cyan()
                );

                match package.authors.len() {
                    0 => println!("  {} no 'authors' found", "warning".yellow()),
                    _ => println!("  authors: {}", package.authors.join(", ")),
                }

                match package.description {
                    None => println!("  {} no 'description' found", "warning".yellow()),
                    Some(description) => println!("  description: {}", description),
                }

                match package.license {
                    None => println!("  {} no 'license' found", "warning".yellow()),
                    Some(license) => println!("  license: {}", license),
                }

                // check dependencies
                const SUBSTRATE_REPO: &str = "git+https://github.com/paritytech/substrate";
                for (name, source) in package.dependencies.iter().filter_map(|d| {
                    d.source
                        .as_ref()
                        .and_then(|s| s.starts_with(SUBSTRATE_REPO).then(|| (&d.name, s)))
                }) {
                    // todo: collect substrate, cumulus, polkadot versions and ensure all match
                    let url = url::Url::parse(&source[4..]).unwrap();
                    for (_, value) in url
                        .query_pairs()
                        .filter(|(parameter, _)| parameter == "branch")
                    {
                        // temp: use last few versions
                        if !["polkadot-v0.9.42", "polkadot-v0.9.43", "polkadot-v1.0.0"]
                            .contains(&value.as_ref())
                        {
                            println!(
                                "  {} {} for '{}' is out of date",
                                "warning".yellow(),
                                value,
                                name
                            )
                        }
                    }
                }
                // TODO: check minimum rust version
            }
        }
        Err(e) => println!("{} could not deserialise: {}", "error".red(), e),
    }
}

fn test() {
    println!("Executing available tests...");

    let _output = Command::new("cargo")
        .arg("test")
        .arg("--no-fail-fast")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

fn benchmark() {
    println!("Executing available benchmarks...");

    let _output = Command::new("cargo")
        .arg("test")
        .arg("--no-default-features")
        .arg("--features=runtime-benchmarks")
        .arg("--no-fail-fast")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

mod clippy {
    use serde::{Deserialize, Serialize};

    pub(super) const EXPECT_UNUSED: &str = "clippy::expect_used";

    #[derive(Serialize, Deserialize)]
    pub(crate) struct Match {
        pub(crate) reason: String,
        pub(crate) message: Option<Message>,
    }

    #[derive(Serialize, Deserialize)]
    pub(crate) struct Message {
        pub(crate) code: Option<Code>,
        pub(crate) level: String,
        pub(crate) message: String,
        pub(crate) spans: Vec<Span>,
        pub(crate) children: Vec<Message>,
    }

    #[derive(Serialize, Deserialize)]
    pub(crate) struct Code {
        pub(crate) code: String,
    }

    #[derive(Serialize, Deserialize)]
    pub(crate) struct Span {
        pub(crate) file_name: String,
        pub(crate) line_start: u16,
        pub(crate) column_start: u16,
        pub(crate) line_end: u16,
        pub(crate) column_end: u16,
        pub(crate) text: Vec<Text>,
    }

    #[derive(Serialize, Deserialize)]
    pub(crate) struct Text {
        pub(crate) text: String,
    }
}

mod manifests {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub(crate) struct Metadata {
        pub(crate) packages: Vec<Package>,
    }

    #[derive(Serialize, Deserialize)]
    pub(crate) struct Package {
        pub(crate) name: String,
        pub(crate) manifest_path: String,
        pub(crate) version: String,
        pub(crate) license: Option<String>,
        pub(crate) license_file: Option<String>,
        pub(crate) description: Option<String>,
        pub(crate) authors: Vec<String>,
        pub(crate) edition: String,
        pub(crate) dependencies: Vec<Dependency>,
    }

    #[derive(Serialize, Deserialize)]
    pub(crate) struct Dependency {
        pub(crate) name: String,
        pub(crate) source: Option<String>,
    }
}

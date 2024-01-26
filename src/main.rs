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

    // Set all configured lints as warning
    let args = clippy::LINTS.map(|l| format!("-W{}", l));
    let output = Command::new("cargo")
        .arg("clippy")
        .arg("--message-format=json")
        .arg("--")
        .args(args)
        .output()
        .unwrap();

    // if output.stderr.len() > 0 {
    //     println!("{}", String::from_utf8_lossy(&output.stderr))
    // }

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

    // Filter and sort matches
    let mut matches: Vec<_> = matches
        .iter()
        .filter_map(|m| m.message.as_ref())
        .filter(|m| m.code.is_some() && !ignored(m))
        .collect();
    matches.sort_by_key(|m| {
        m.spans
            .get(0)
            .map(|s| (&s.file_name, s.line_start, s.column_start))
    });
    // Output results
    for message in matches {
        print!(
            "{} {} {}",
            match message.level.as_str() {
                "warning" => message.level.yellow(),
                "error" => message.level.red(),
                _ => message.level.normal(),
            },
            message.code.as_ref().map_or("".into(), |c| {
                match c.code.starts_with("clippy::") {
                    true => {
                        let url = format!(
                            "https://rust-lang.github.io/rust-clippy/master/#/{}",
                            c.code.replace("clippy::", "")
                        );
                        Link::new(&c.code, &url).to_string().cyan()
                    }
                    false => c.code.as_str().into(),
                }
            }),
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
    const IGNORED: [&str; 7] = [
        "construct_runtime!",
        "#[frame_support::pallet]",
        "#[pallet::call]",
        "#[pallet::error]",
        "#[pallet::event]",
        "#[pallet::pallet]",
        "#[pallet::storage]",
    ];
    message.spans.iter().any(|s| {
        s.text
            .iter()
            .any(|t| IGNORED.iter().any(|i| t.text.contains(i)))
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

                // Check for common metadata: https://rust-lang.github.io/api-guidelines/documentation.html#cargotoml-includes-all-common-metadata-c-metadata
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

                match package.repository {
                    None => println!("  {} no 'repository' found", "warning".yellow()),
                    Some(repository) => println!("  repository: {}", repository),
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

    // Source: https://rust-lang.github.io/rust-clippy/master/
    pub(super) const LINTS: [&str; 124] = [
        "clippy::alloc_instead_of_core",
        "clippy::allow_attributes_without_reason",
        "clippy::arithmetic_side_effects",
        "clippy::as_underscore",
        "clippy::assertions_on_result_states",
        "clippy::bool_to_int_with_if",
        "clippy::branches_sharing_code",
        "clippy::cargo_common_metadata",
        "clippy::cast_lossless",
        "clippy::cast_possible_truncation",
        "clippy::cast_possible_wrap",
        "clippy::cast_precision_loss",
        "clippy::cast_sign_loss",
        "clippy::checked_conversions",
        "clippy::cloned_instead_of_copied",
        "clippy::cognitive_complexity",
        "clippy::dbg_macro",
        "clippy::default_trait_access",
        "clippy::derive_partial_eq_without_eq",
        "clippy::else_if_without_else",
        "clippy::empty_structs_with_brackets",
        "clippy::enum_glob_use",
        "clippy::equatable_if_let",
        "clippy::exit",
        "clippy::expect_used",
        "clippy::explicit_into_iter_loop",
        "clippy::explicit_iter_loop",
        "clippy::fallible_impl_from",
        "clippy::filter_map_next",
        "clippy::flat_map_option",
        "clippy::float_arithmetic",
        "clippy::float_cmp",
        "clippy::float_cmp_const",
        "clippy::format_push_string",
        "clippy::get_unwrap",
        "clippy::if_not_else",
        "clippy::if_then_some_else_none",
        "clippy::indexing_slicing",
        "clippy::integer_division",
        "clippy::implicit_clone",
        "clippy::inconsistent_struct_constructor",
        "clippy::inefficient_to_string",
        "clippy::invalid_upcast_comparisons",
        "clippy::items_after_statements",
        "clippy::iter_on_empty_collections",
        "clippy::iter_on_single_items",
        "clippy::iter_with_drain",
        "clippy::large_digit_groups",
        "clippy::large_include_file",
        "clippy::large_stack_arrays",
        "clippy::large_types_passed_by_value",
        "clippy::let_underscore_must_use",
        "clippy::linkedlist",
        "clippy::lossy_float_literal",
        "clippy::manual_clamp",
        "clippy::manual_ok_or",
        "clippy::manual_string_new",
        "clippy::many_single_char_names",
        "clippy::map_err_ignore",
        "clippy::map_unwrap_or",
        "clippy::match_bool",
        "clippy::match_on_vec_items",
        "clippy::match_same_arms",
        "clippy::match_wild_err_arm",
        "clippy::match_wildcard_for_single_variants",
        "clippy::maybe_infinite_iter",
        "clippy::mismatching_type_param_order",
        "clippy::mixed_read_write_in_expression",
        "clippy::module_name_repetitions",
        "clippy::multiple_crate_versions",
        "clippy::multiple_inherent_impl",
        "clippy::needless_collect",
        "clippy::needless_continue",
        "clippy::needless_for_each",
        "clippy::needless_pass_by_value",
        "clippy::no_effect_underscore_binding",
        "clippy::nonstandard_macro_braces",
        "clippy::option_if_let_else",
        "clippy::option_option",
        "clippy::or_fun_call",
        "clippy::panic",
        "clippy::panic_in_result_fn",
        "clippy::partial_pub_fields",
        "clippy::print_stderr",
        "clippy::print_stdout",
        "clippy::pub_use",
        "clippy::range_minus_one",
        "clippy::range_plus_one",
        "clippy::redundant_clone",
        "clippy::redundant_closure_for_method_calls",
        "clippy::redundant_pub_crate",
        "clippy::ref_binding_to_reference",
        "clippy::ref_option_ref",
        "clippy::rest_pat_in_fully_bound_structs",
        "clippy::same_functions_in_if_condition",
        "clippy::same_name_method",
        "clippy::similar_names",
        "clippy::string_slice",
        "clippy::string_to_string",
        "clippy::struct_excessive_bools",
        "clippy::suspicious_operation_groupings",
        "clippy::todo",
        "clippy::too-many-lines",
        "clippy::trait_duplication_in_bounds",
        "clippy::trivial_regex",
        "clippy::trivially_copy_pass_by_ref",
        "clippy::try_err",
        "clippy::type_repetition_in_bounds",
        "clippy::unimplemented",
        "clippy::uninlined_format_args",
        "clippy::unnecessary_join",
        "clippy::unnecessary_self_imports",
        "clippy::unnecessary_wraps",
        "clippy::unneeded_field_pattern",
        "clippy::unnested_or_patterns",
        "clippy::unreachable",
        "clippy::unreadable_literal",
        "clippy::unused_self",
        "clippy::unwrap_in_result",
        "clippy::unwrap_used",
        "clippy::use_debug",
        "clippy::use_self",
        "clippy::useless_let_if_seq",
        "clippy::wildcard_enum_match_arm",
    ];

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
        pub(crate) repository: Option<String>,
        pub(crate) categories: Vec<String>,
        pub(crate) keywords: Vec<String>,
        pub(crate) edition: String,
        pub(crate) dependencies: Vec<Dependency>,
    }

    #[derive(Serialize, Deserialize)]
    pub(crate) struct Dependency {
        pub(crate) name: String,
        pub(crate) source: Option<String>,
    }
}

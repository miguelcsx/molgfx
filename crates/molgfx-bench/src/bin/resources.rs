//! Allocator and optional isolated OS RSS measurements for scene authoring.
//!
//! `resources [CASE]` reports heap regions at all sizes; it does not claim RSS.
//! `resources --isolated OUT [CASE]` runs three fresh children per case/size,
//! preserving JSONL and Darwin `time -l` output outside compilation.
//! Rust allocator regions exclude fixtures; whole-child RSS includes fixtures,
//! runtime and native mappings. Neither figure is logical device residency.

use molgfx_bench::{process::measure_process, resources};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

#[global_allocator]
static ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> = &stats_alloc::INSTRUMENTED_SYSTEM;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [] => print_records("all"),
        [case] if case != "--isolated" => print_records(case),
        [flag, case, residues] if flag == "--child" => {
            let record = resources::run_one(case, residues.parse()?)?;
            println!("{}", serde_json::to_string(&record)?);
            Ok(())
        }
        [flag, output] if flag == "--isolated" => isolated(Path::new(output), "all"),
        [flag, output, case] if flag == "--isolated" => isolated(Path::new(output), case),
        _ => Err(io::Error::other("usage: resources [CASE] | --isolated OUT [CASE]").into()),
    }
}

fn print_records(case: &str) -> Result<(), Box<dyn Error>> {
    for record in resources::run(case)? {
        println!("{}", serde_json::to_string(&record)?);
    }
    Ok(())
}

fn isolated(output: &Path, selected: &str) -> Result<(), Box<dyn Error>> {
    if selected != "all" {
        resources::case_name(selected)?;
    }
    let executable = std::env::current_exe()?;
    for case in resources::CASES
        .into_iter()
        .filter(|case| selected == "all" || *case == selected)
    {
        for residues in resources::SIZES {
            for trial in 1..=3 {
                let directory = output
                    .join(case)
                    .join(residues.to_string())
                    .join(trial.to_string());
                let arguments = ["--child".to_owned(), case.to_owned(), residues.to_string()];
                let measurement = measure_process(&executable, &arguments, &directory)?;
                let summary = File::create_new(directory.join("process.json"))?;
                serde_json::to_writer(summary, &measurement)?;
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "case": case, "residues": residues, "trial": trial,
                        "rss_scope": "whole isolated child including fixtures and native mappings",
                        "rss_unit": "bytes", "process": measurement,
                    }))?
                );
            }
        }
    }
    Ok(())
}

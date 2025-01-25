use std::{fs::read_to_string, path::PathBuf};

use clap::Parser;
use log::{debug, info};
use once_cell::sync::Lazy;
use regex::Regex;

/// Add bricklayers to Prusa and Orca slicer gcode.
#[derive(Parser)]
#[command(version, about, long_about)]
struct Cli {
    // #[arg(short, long)]
    // layer_height: f64,
    /// Multiplies the extrusions of the shifted layers so you can use it to
    /// probably increase strength (has yet to be tested).
    #[arg(short, long, default_value_t = 1.0)]
    extrusion_multiplier: f64,
    file: PathBuf,
    /// File to write processed gcode to.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Set this to overwrite input file, useful for automatic post-processing
    /// such as inside your slicer settings. Otherwise, this will create a new
    /// file with an prepended extension `brickd`, or use your provided output
    /// filename.
    #[arg(short = 'w', long)]
    overwrite: bool,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    info!("Input file: {:?}", &cli.file);
    let gcode = read_to_string(&cli.file).unwrap();

    let new_gcode = process(
        &gcode,
        // cli.layer_height,
        cli.extrusion_multiplier,
    );

    let out_path = cli.output.unwrap_or(if cli.overwrite {
        cli.file
    } else {
        cli.file.with_extension(".brickd.gcode")
    });
    std::fs::write(&out_path, new_gcode).expect("could not write new gcode file");
    info!("Output file: {:?}", out_path)
}

#[derive(PartialEq)]
enum PerimeterType {
    External,
    Internal,
    None,
}

fn process(
    gcode: &str,
    // layer_height: f64,
    extrusion_multiplier: f64,
) -> String {
    let mut current_layer = 0;
    let mut current_z = 0f64;
    let mut perimeter_type = PerimeterType::None;
    let mut perimeter_block_count = 0;
    let mut inside_perimeter_block = false;
    let mut is_shifted = false;

    let layer_height: f64 = gcode
        .lines()
        .find(|l| l.starts_with("; layer_height = "))
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .parse()
        .unwrap();
    let z_shift = layer_height * 0.5;
    info!("Z-shift: {z_shift}mm, Layer height: {layer_height}mm");

    let total_layers = gcode.lines().filter(|l| l.starts_with("G1 Z")).count();

    static Z_MATCH: Lazy<Regex> = Lazy::new(|| Regex::new(r"Z([-\d.]+)").unwrap());
    static E_MATCH: Lazy<Regex> = Lazy::new(|| Regex::new(r"E([-\d.]+)").unwrap());
    let mut new_lines = vec![];
    for mut line in gcode.lines().map(|l| l.to_string()) {
        // Detect layer changes
        let z_match = Z_MATCH.captures(&line);
        if line.starts_with("G1 Z") {
            if let Some(cap) = z_match {
                current_z = cap[1].parse().expect("failed to parse current_z");
                current_layer = (current_z / layer_height).trunc() as u64;

                perimeter_block_count = 0;
                debug!("Layer {current_layer} detected at Z={current_z:0.3}");
            }
            new_lines.push(line);
            continue;
        }

        // Detect perimeter types from PrusaSlicer comments
        if line.contains(";TYPE:External perimeter") || line.contains(";TYPE:Outer wall") {
            perimeter_type = PerimeterType::External;
            inside_perimeter_block = false;
            debug!("External perimeter detected at layer {current_layer}");
        } else if line.contains(";TYPE:Perimeter") || line.contains(";TYPE:Inner wall") {
            perimeter_type = PerimeterType::Internal;
            inside_perimeter_block = false;
            debug!("Internal perimeter block started at layer {current_layer}");
        } else if line.contains(";TYPE:") {
            perimeter_type = PerimeterType::None;
            inside_perimeter_block = false;
        }

        // Group lines into perimeter blocks
        if perimeter_type == PerimeterType::Internal
            && line.starts_with("G1")
            && line.contains('X')
            && line.contains('Y')
            && line.contains('E')
        {
            // Start a new perimeter block if not already inside one
            if !inside_perimeter_block {
                perimeter_block_count += 1;
                inside_perimeter_block = true;
                debug!(
                    "Perimeter block #{perimeter_block_count} detected at layer {current_layer}"
                );

                // Insert the corresponding Z height for this block
                is_shifted = false;
                if perimeter_block_count % 2 == 1 {
                    let adjusted_z = current_z + z_shift;
                    new_lines.push(format!(
                        "G1 Z{adjusted_z:0.3} ; Shifted Z for block #{perimeter_block_count}"
                    ));
                    debug!(
                        "Inserted G1 Z{adjusted_z:0.3} for shifter perimeter block #{perimeter_block_count}"
                    );
                    is_shifted = true;
                } else {
                    // Reset to the true layer height for even-numbered blocks
                    new_lines.push(format!(
                        "G1 Z{current_z:0.3} ; Reset Z for block #{perimeter_block_count}"
                    ));
                    debug!(
                        "Inserted G1 Z{current_z:0.3} for non-shifted perimeter block #{perimeter_block_count}"
                    );
                }
            }

            // Adjust extrusion (`E` values) for shifted blocks on the first and last layer
            if is_shifted {
                let e_match = E_MATCH.captures(&line);
                if let Some(cap) = e_match {
                    let e_value: f64 = cap[1].parse().expect("failed to parse e_value");
                    if current_layer == 0 {
                        let new_e_value = e_value * 1.5;
                        line = E_MATCH
                            .replace_all(&line, format!("E{new_e_value:0.5}"))
                            .trim()
                            .to_string();
                        line.push_str(&format!(
                            " ; Adjusted E for first layer, block #{perimeter_block_count}"
                        ));
                        debug!(
                            "Multiplying E value by 1.5 on first layer (shifted block): {e_value:0.5} -> {new_e_value:0.5}"
                        );
                    } else if current_layer == (total_layers - 1) as u64 {
                        let new_e_value = e_value * 0.5;
                        line = E_MATCH
                            .replace_all(&line, format!("E{new_e_value:0.5}"))
                            .trim()
                            .to_string();
                        line.push_str(&format!(
                            " ; Adjusted E for last layer, block #{perimeter_block_count}"
                        ));
                        debug!(
                            "Multiplying E value by 0.5 on last layer (shifted block): {e_value:0.5} -> {new_e_value:0.5}"
                        );
                    } else {
                        let new_e_value = e_value * extrusion_multiplier;
                        line = E_MATCH
                            .replace_all(&line, format!("E{new_e_value:0.5}"))
                            .trim()
                            .to_string();
                        line.push_str(&format!(
                            " ; Adjusted E for extrusionMultiplier, block #{perimeter_block_count}"
                        ));
                        debug!(
                            "Multiplying E value by extrusion multiplier: {e_value:0.5} -> {new_e_value:0.5}"
                        );
                    }
                }
            }
        } else if perimeter_type == PerimeterType::Internal
            && line.starts_with("G1")
            && line.contains('X')
            && line.contains('Y')
            && line.contains('F')
        {
            inside_perimeter_block = false;
        }
        new_lines.push(line);
    }
    new_lines.join("\n")
}

# Bricklayers
This is a Rust version of [TengerTechnologies/Bricklayers](https://github.com/TengerTechnologies/Bricklayers) to add Brick layers to Prusaslicer and Orcaslicer.
(As of now it doesn't work with Bambu printers)

You'll need to have Cargo installed. [rustup.rs](https://rustup.rs)

Install by cloning this repository and running `cargo install --path .` inside the repo.

<!-- Alternatively, download a release for your platform from Releases. -->

In your slicer, go to "Print Settings", "Output options", "Post-processing scripts", and add:

For Linux / MacOS users,
```
/path/to/your/cargo/bindir/bricklayers -w
```

For Windows,
```
\path\to\your\cargo\bindir\bricklayers -w
```

You may optionally add the argument `--extrusion-multiplier`. This multiplies the extrusions of the shifter layers to potentially increase strength (untested).

Here is a video about the script.

[![IMAGE ALT TEXT HERE](https://img.youtube.com/vi/EqRdQOoK5hc/0.jpg)](https://www.youtube.com/watch?v=EqRdQOoK5hc)

Here are some benchmarks I ran on random gcode I had lying around:
![screenshot showing benchmark results](./benchmarks/20250125.png)

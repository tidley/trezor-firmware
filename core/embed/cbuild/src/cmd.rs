use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Default)]
pub struct InputFiles {
    files: Vec<PathBuf>,
}

impl InputFiles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_files_from_folder(
        &mut self,
        folder: impl AsRef<Path>,
        extension: &str,
    ) -> std::io::Result<()> {
        let extension = extension.trim_start_matches('.');

        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            let matches_extension = path
                .extension()
                .is_some_and(|ext| ext == std::ffi::OsStr::new(extension));

            if !matches_extension {
                continue;
            }

            self.files.push(path);
        }

        self.files.sort();
        self.files.dedup();

        Ok(())
    }

    pub fn exclude_file(&mut self, file_name: &str) {
        self.files
            .retain(|path| path.file_name().and_then(|name| name.to_str()) != Some(file_name));
    }

    pub fn as_paths(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn as_path_refs(&self) -> Vec<&PathBuf> {
        self.files.iter().collect()
    }
}

pub fn inputs_newer_than_outputs<I, O>(inputs: &[I], outputs: &[O]) -> bool
where
    I: AsRef<Path>,
    O: AsRef<Path>,
{
    let newest_input = inputs
        .iter()
        .map(AsRef::as_ref)
        .filter_map(|path| fs::metadata(path).ok())
        .filter_map(|meta| meta.modified().ok())
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let oldest_output = outputs
        .iter()
        .filter_map(|path| fs::metadata(path).ok())
        .filter_map(|meta| meta.modified().ok())
        .min();

    match oldest_output {
        Some(oldest) => newest_input > oldest,
        None => true,
    }
}

#[allow(dead_code)]
pub fn run_with_dep_tracking<I, O, F>(inputs: &[I], outputs: &[O], args: Option<&str>, run_once: F)
where
    I: AsRef<Path>,
    O: AsRef<Path>,
    F: FnOnce(),
{
    if outputs.is_empty() {
        panic!("run_with_dep_tracking requires at least one output file");
    }

    // Dependency file is named after the first output file with `.dep` extension
    let dep_path = PathBuf::from(format!("{}.dep", outputs[0].as_ref().display()));

    // List program arguments
    let mut dep = String::new();
    dep.push_str("--args--\n");

    if let Some(args) = args {
        dep.push_str(args);
        if !dep.ends_with('\n') {
            dep.push('\n');
        }
    }

    // List inputs for dependency tracking
    dep.push_str("--inputs--\n");
    for input in inputs {
        dep.push_str(&input.as_ref().to_string_lossy());
        dep.push('\n');
    }

    // List outputs for dependency tracking
    dep.push_str("--outputs--\n");
    for output in outputs {
        dep.push_str(&output.as_ref().to_string_lossy());
        dep.push('\n');
    }

    for input in inputs {
        println!("cargo:rerun-if-changed={}", input.as_ref().display());
    }

    let dep_changed = !matches!(fs::read_to_string(&dep_path), Ok(content) if content == dep);

    let output_missing = outputs.iter().any(|out| !out.as_ref().exists());

    let outputs_stale = inputs_newer_than_outputs(inputs, outputs);

    let should_run = dep_changed || output_missing || outputs_stale;

    if should_run {
        let _ = fs::remove_file(&dep_path);

        if let Some(parent) = dep_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create dep file directory");
        }

        run_once();

        fs::write(&dep_path, dep).expect("Failed to write dep file");
    }
}

// Runs an external tool and tracks its inputs/outputs in a `.dep` file.
// Re-runs when metadata changes, any output is missing, or an input is newer.
pub fn run_cmd<I, O>(cmd: &mut std::process::Command, inputs: &[I], outputs: &[O])
where
    I: AsRef<Path>,
    O: AsRef<Path>,
{
    let program_path = PathBuf::from(cmd.get_program());

    let mut args = String::new();

    args.push_str(&program_path.to_string_lossy());
    args.push('\n');

    for arg in cmd.get_args() {
        args.push_str(&arg.to_string_lossy());
        args.push('\n');
    }

    // Combine program and args with input files for dependency tracking
    let mut all_inputs: Vec<&Path> = inputs.iter().map(AsRef::as_ref).collect();
    all_inputs.push(program_path.as_path());

    run_with_dep_tracking(&all_inputs, outputs, Some(args.as_str()), || {
        let output = cmd.output().expect("Failed to execute external tool");

        if !output.status.success() {
            panic!(
                "external tool failed with status {}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    });
}

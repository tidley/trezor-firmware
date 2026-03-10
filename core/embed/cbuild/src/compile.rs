use pathdiff::diff_paths;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

#[derive(Default, Clone)]
pub struct CLibrary {
    sources: Vec<PathBuf>,
    flags: Vec<String>,
    public_flags: Vec<String>,
    defines: Vec<(String, Option<String>)>,
    public_defines: Vec<(String, Option<String>)>,
    includes: Vec<PathBuf>,
    public_includes: Vec<PathBuf>,
    public_libs: Vec<PathBuf>,
}

// Converts `path` to a path relative to `CARGO_MANIFEST_DIR` when possible.
//
// Relative inputs are first interpreted as `CARGO_MANIFEST_DIR/<path>`.
// Absolute inputs are used as-is.
//
// If a relative path from `CARGO_MANIFEST_DIR` can be computed, it is
// returned; otherwise the original absolute path is returned unchanged.
//
// Note: this is lexical path computation (no filesystem access or symlink
// resolution).
fn make_path_relative_to_manifest(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        join_paths_lexically(&manifest_dir, path)
    };

    let rel_path = diff_paths(&abs_path, &manifest_dir).unwrap_or(abs_path);

    if rel_path.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        rel_path
    }
}

// Joins `base` with `relative` and normalizes components lexically.
//
// This collapses repeated separators, removes `.` components, and handles
// `..` by popping one previously collected component when possible.
//
// Note: this is purely lexical normalization (no filesystem access, no
// symlink resolution), and a `..` with nothing to pop is ignored.
fn join_paths_lexically(base: impl AsRef<Path>, relative: impl AsRef<Path>) -> PathBuf {
    let joined_path = base.as_ref().join(relative.as_ref());
    joined_path
        .components()
        .fold(PathBuf::new(), |mut acc, comp| {
            match comp {
                Component::ParentDir => {
                    acc.pop();
                }
                Component::CurDir => {}
                other => acc.push(other.as_os_str()),
            }
            acc
        })
}

fn emit_deps(content: &str) {
    for line in content.lines() {
        // Clean up the .d format (target: dependency1 dependency2 ...)
        let parts = line.split(':').next_back().unwrap_or("");
        for path in parts.split_whitespace() {
            if path != "\\" {
                println!("cargo:rerun-if-changed={}", path);
            }
        }
    }
}

#[derive(Clone)]
struct CompileUnit {
    index: usize,
    src_path: PathBuf,
    obj_path: PathBuf,
    dep_path: PathBuf,
}

fn determine_parallel_jobs(unit_count: usize) -> usize {
    env::var("CBUILD_JOBS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .or_else(|| {
            env::var("NUM_JOBS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
        })
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
        })
        .max(1)
        .min(unit_count.max(1))
}

fn is_external_source_path(path: &Path) -> bool {
    path.is_absolute() || matches!(path.components().next(), Some(Component::ParentDir))
}

fn external_object_subpath(src_path: &Path) -> PathBuf {
    let mut path = PathBuf::from("__external");
    for component in src_path.components() {
        if let Component::Normal(part) = component {
            path.push(part);
        }
    }
    path
}

impl CLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a single source file to the library.
    ///
    /// The path should be relative to the crate root or absolute.
    pub fn add_source(&mut self, src: impl AsRef<Path>) {
        let src = make_path_relative_to_manifest(src);
        self.sources.push(src);
    }

    pub fn get_sources(&self) -> Vec<&PathBuf> {
        self.sources.iter().collect()
    }

    /// Adds multiple source files to the library.
    ///
    /// The paths should be relative to the crate root or absolute.
    pub fn add_sources<I, P>(&mut self, sources: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        for src in sources {
            self.add_source(src);
        }
    }

    pub fn add_sources_from_folder<I, P>(&mut self, prefix: impl AsRef<Path>, sources: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        for src in sources {
            let full_path = PathBuf::from(prefix.as_ref()).join(src.as_ref());
            self.add_source(full_path);
        }
    }

    /// Adds include directory to the library.
    ///
    /// The path should be relative to the crate root or absolute.
    pub fn add_include(&mut self, path: impl AsRef<Path>) {
        let path = make_path_relative_to_manifest(path);
        if !self.includes.contains(&path) {
            self.includes.push(path);
        }
    }

    /// Adds multiple include directories to the library.
    ///
    /// The paths should be relative to the crate root or absolute.
    pub fn add_includes<I, P>(&mut self, paths: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        for path in paths {
            self.add_include(path);
        }
    }

    /// Adds a preprocessor define to the library.
    ///
    /// `value` is optional and can be used for defines like `-DNAME=value`.
    /// If `value` is `None`, it will be treated as `-DNAME`.
    pub fn add_define(&mut self, name: &str, value: Option<&str>) {
        self.defines
            .push((name.to_string(), value.map(|v| v.to_string())));
    }

    /// Adds multiple preprocessor defines to the library.
    ///
    pub fn add_defines<I, N, V>(&mut self, defines: I)
    where
        I: IntoIterator<Item = (N, Option<V>)>,
        N: AsRef<str>,
        V: AsRef<str>,
    {
        for (name, value) in defines {
            self.add_define(name.as_ref(), value.as_ref().map(|v| v.as_ref()));
        }
    }

    pub fn add_public_include(&mut self, path: impl AsRef<Path>) {
        let path = make_path_relative_to_manifest(path);
        if !self.public_includes.contains(&path) {
            self.public_includes.push(path);
        }
    }

    pub fn add_public_includes<I, P>(&mut self, paths: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        for path in paths {
            self.add_public_include(path);
        }
    }

    pub fn add_public_define(&mut self, name: &str, value: Option<&str>) {
        self.public_defines
            .push((name.to_string(), value.map(|v| v.to_string())));
    }

    pub fn add_public_defines<I, N, V>(&mut self, defines: I)
    where
        I: IntoIterator<Item = (N, Option<V>)>,
        N: AsRef<str>,
        V: AsRef<str>,
    {
        for (name, value) in defines {
            self.add_public_define(name.as_ref(), value.as_ref().map(|v| v.as_ref()));
        }
    }

    pub fn add_flag(&mut self, flag: &str) {
        if !self.flags.contains(&flag.to_string()) {
            self.flags.push(flag.to_string());
        }
    }

    pub fn add_flags(&mut self, flags: &[&str]) {
        for flag in flags {
            self.add_flag(flag);
        }
    }

    pub fn add_public_flag(&mut self, flag: &str) {
        if !self.public_flags.contains(&flag.to_string()) {
            self.public_flags.push(flag.to_string());
        }
    }

    pub fn add_public_flags<I, F>(&mut self, flags: I)
    where
        I: IntoIterator<Item = F>,
        F: AsRef<str>,
    {
        for flag in flags {
            self.add_public_flag(flag.as_ref());
        }
    }

    pub fn add_public_lib(&mut self, lib: impl AsRef<Path>) {
        let lib = lib.as_ref().to_path_buf();
        if !self.public_libs.contains(&lib) {
            self.public_libs.push(lib);
        }
    }

    /// Use another C library defined in a different crate.
    ///
    /// This will automatically import the public include paths, defines,
    /// and flags from that crate.
    pub fn use_lib(&mut self, crate_name: &str) {
        self.add_public_lib(crate_name);

        let get_c_public = |crate_name: &str, kind: &str| -> String {
            std::env::var(format!(
                "DEP_{}_PUBLIC_C_{}",
                crate_name.to_uppercase(),
                kind
            ))
            .unwrap_or_default()
        };

        let public_inc = get_c_public(crate_name, "INCLUDES");

        for dir in public_inc.split(';').filter(|path| !path.is_empty()) {
            self.add_public_include(dir);
        }

        let public_defines = get_c_public(crate_name, "DEFINES");

        for def in public_defines.split(';').filter(|def| !def.is_empty()) {
            let parts: Vec<&str> = def.splitn(2, '=').collect();
            let name = parts[0];
            let value = if parts.len() > 1 {
                Some(parts[1])
            } else {
                None
            };
            self.add_public_define(name, value);
        }

        let public_flags = get_c_public(crate_name, "FLAGS");

        for flag in public_flags.split(';').filter(|flag| !flag.is_empty()) {
            self.add_public_flag(flag);
        }

        let public_libs = get_c_public(crate_name, "LIBS");

        for lib in public_libs.split(';').filter(|lib| !lib.is_empty()) {
            self.add_public_lib(lib);
        }
    }

    fn export_public_link_libs(&self) {
        self.public_libs.iter().for_each(|lib| {
            println!("cargo:rustc-link-lib=static={}", lib.to_string_lossy());
        });
    }

    fn export_public_c_libs_metadata(&self, lib_name: &str) {
        let mut exported_public_libs = self.public_libs.clone();
        let lib_name_path = PathBuf::from(lib_name);
        if !exported_public_libs.contains(&lib_name_path) {
            exported_public_libs.push(lib_name_path);
        }
        println!(
            "cargo::metadata=public_c_libs={}",
            exported_public_libs
                .iter()
                .map(|lib| lib.to_string_lossy())
                .collect::<Vec<_>>()
                .join(";")
        );
    }

    fn export_public_c_includes_metadata(&self) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let public_includes = self
            .public_includes
            .iter()
            .map(|dir| {
                if dir.is_absolute() {
                    dir.to_string_lossy().to_string()
                } else {
                    join_paths_lexically(&manifest_dir, dir)
                        .to_string_lossy()
                        .to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(";");
        println!("cargo::metadata=public_c_includes={}", public_includes);
    }

    fn export_public_c_defines_metadata(&self) {
        let public_defines = self
            .public_defines
            .iter()
            .map(|(name, value)| {
                if let Some(val) = value {
                    format!("{}={}", name, val)
                } else {
                    name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(";");
        println!("cargo::metadata=public_c_defines={}", public_defines);
    }

    fn export_public_c_flags_metadata(&self) {
        let public_flags = self.public_flags.join(";");
        println!("cargo::metadata=public_c_flags={}", public_flags);
    }

    fn export_c_compiler_includes_metadata(&self) {
        let compiler = cc::Build::new().get_compiler();
        let compile_result = compiler
            .to_command()
            .arg("-E")
            .arg("-Wp,-v")
            .arg("-")
            .output()
            .expect("compiler failed to execute");
        if !compile_result.status.success() {
            panic!("compiler failed");
        }

        let compiler_output =
            String::from_utf8(compile_result.stderr).expect("compiler returned invalid output");

        let compiler_includes = compiler_output
            .lines()
            .skip_while(|s| !s.contains("search starts here:"))
            .take_while(|s| !s.contains("End of search list."))
            .filter(|s| s.starts_with(' '))
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
            .join(";");

        println!("cargo::metadata=c_compiler_includes={}", compiler_includes);
    }

    fn export_build_metadata(&self, lib_name: &str) {
        self.export_public_link_libs();
        self.export_public_c_libs_metadata(lib_name);
        self.export_public_c_includes_metadata();
        self.export_public_c_defines_metadata();
        self.export_public_c_flags_metadata();
        // Export compiler default include paths (for bindgen)
        //TODO!@# maybe do not export in emulator build
        self.export_c_compiler_includes_metadata();
    }

    pub fn build(self: &CLibrary) {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let mut compile_units = Vec::new();

        for (index, src) in self.sources.iter().enumerate() {
            let abs_path = if src.is_absolute() {
                src.clone()
            } else {
                join_paths_lexically(&manifest_dir, src)
            };

            let obj_path = out_dir.join(
                if is_external_source_path(src) {
                    external_object_subpath(&abs_path)
                } else {
                    src.clone()
                }
                .with_extension("o"),
            );

            let dep_path = obj_path.with_extension("d");

            if let Some(parent) = obj_path.parent() {
                fs::create_dir_all(parent).expect("Failed to create output directory");
            }

            compile_units.push(CompileUnit {
                index,
                src_path: abs_path,
                obj_path,
                dep_path,
            });
        }

        let compile_config = Arc::new(self.clone());

        let jobs = determine_parallel_jobs(compile_units.len());

        let (work_tx, work_rx) = mpsc::channel::<CompileUnit>();
        let work_rx = Arc::new(Mutex::new(work_rx));
        let (result_tx, result_rx) =
            mpsc::channel::<Result<(usize, PathBuf, Option<String>), String>>();

        let mut workers = Vec::with_capacity(jobs);
        for _ in 0..jobs {
            let work_rx = Arc::clone(&work_rx);
            let result_tx = result_tx.clone();
            let compile_config = Arc::clone(&compile_config);

            workers.push(thread::spawn(move || {
                loop {
                    let unit = {
                        let receiver = work_rx.lock().expect("Failed to lock work queue");
                        receiver.recv()
                    };

                    let Ok(unit) = unit else {
                        break;
                    };

                    let result = compile_config.compile_source_unit(&unit);

                    if result_tx.send(result).is_err() {
                        break;
                    }
                }
            }));
        }

        for unit in compile_units {
            work_tx
                .send(unit)
                .expect("Failed to submit compilation task");
        }
        drop(work_tx);
        drop(result_tx);

        let mut compiled = Vec::new();
        while let Ok(result) = result_rx.recv() {
            match result {
                Ok(entry) => compiled.push(entry),
                Err(err) => panic!("{}", err),
            }
        }

        for worker in workers {
            worker.join().expect("Failed to join compilation worker");
        }

        compiled.sort_by_key(|(index, _, _)| *index);

        let mut objects = Vec::with_capacity(compiled.len());
        for (_, obj_path, dep_content) in compiled {
            if let Some(content) = dep_content {
                emit_deps(&content);
            }
            objects.push(obj_path);
        }

        // Batch all objects into ONE static library
        let mut lib = cc::Build::new();
        for obj in objects {
            lib.object(obj);
        }

        let lib_name =
            env::var("CARGO_MANIFEST_LINKS").unwrap_or_else(|_| "native_part".to_string());

        lib.compile(&lib_name);

        self.export_build_metadata(&lib_name);
    }

    pub fn create_configured_build(&self) -> cc::Build {
        let mut build = cc::Build::new();
        build.warnings(false);

        for flag in &self.flags {
            build.flag(flag);
        }

        for flag in &self.public_flags {
            build.flag(flag);
        }

        for dir in &self.public_includes {
            build.include(dir);
        }

        for dir in &self.includes {
            build.include(dir);
        }

        for def in &self.public_defines {
            build.define(&def.0, def.1.as_deref());
        }

        for def in &self.defines {
            build.define(&def.0, def.1.as_deref());
        }

        build
    }

    fn compile_source_unit(
        &self,
        unit: &CompileUnit,
    ) -> Result<(usize, PathBuf, Option<String>), String> {
        let build = self.create_configured_build();

        let mut cmd = build.get_compiler().to_command();
        cmd.arg("-c")
            .arg("-MMD")
            .arg("-MF")
            .arg(&unit.dep_path)
            .arg("-o")
            .arg(&unit.obj_path)
            .arg(&unit.src_path);

        let output = cmd.output().map_err(|err| {
            format!(
                "Failed to execute compiler for {:?}: {}",
                unit.src_path, err
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let details = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "no compiler output".to_string()
            };

            return Err(format!(
                "Failed to compile {:?}: {}",
                unit.src_path, details
            ));
        }

        let dep_content = fs::read_to_string(&unit.dep_path).ok();
        Ok((unit.index, unit.obj_path.clone(), dep_content))
    }

    pub fn build_upydef(&self) -> Vec<PathBuf> {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

        let mut generated_files = Vec::new();

        for src in &self.sources {
            let abs_path = if src.is_absolute() {
                src.clone()
            } else {
                join_paths_lexically(&manifest_dir, src)
            };

            let obj_path = out_dir.join(
                if is_external_source_path(src) {
                    external_object_subpath(&abs_path)
                } else {
                    src.clone()
                }
                .with_extension("o"),
            );

            let output = obj_path.with_extension("upydef");

            let mut cmd = self.create_configured_build().get_compiler().to_command();
            cmd.args(["-E", "-DNO_QSTR", "-DN_X64", "-DN_X86", "-DN_THUMB"])
                .arg(&abs_path)
                .arg("-o")
                .arg(&output);

            let inputs = vec![&abs_path];
            let outputs = vec![&output];
            crate::run_cmd(&mut cmd, &inputs, &outputs);

            generated_files.push(output);
        }

        generated_files
    }

    /// Preprocesses a C header/source file using the configured C compiler.
    ///
    /// This runs the compiler in preprocessor mode (`-E`) and writes output
    /// to `output`.
    pub fn preprocess_file(&self, input: &PathBuf, output: &PathBuf) {
        let mut cmd = self.create_configured_build().get_compiler().to_command();
        cmd.arg("-E")
            .arg("-DQSTR_PROCESSING")
            .arg(input)
            .arg("-o")
            .arg(output);
        crate::run_cmd(&mut cmd, &[input], &[output]);
    }
}

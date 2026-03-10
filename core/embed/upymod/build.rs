use std::{
    env,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
};

fn measure_time<T, F>(label: &str, f: F) -> T
where
    F: FnOnce() -> T,
{
    let start_time = std::time::Instant::now();
    let result = f();
    let duration = start_time.elapsed();
    eprintln!("{}: {:.2?}", label, duration);
    result
}

fn main() {
    let mut lib = cbuild::CLibrary::new();

    let mpy_folder = "../../vendor/micropython";

    lib.use_lib("io");

    lib.add_public_include("../projects/unix");

    lib.add_public_include(mpy_folder);

    lib.add_public_define("BITCOIN_ONLY", Some("1")); // !@# add feature flag

    lib.add_public_define("UI_LAYOUT_BOLT", None); // !@# temp hack for modtrezorutils.c, add feature flag

    lib.add_public_define(
        "MICROPY_ENABLE_SOURCE_LINE",
        Some(
            cfg!(feature = "enable_source_line")
                .then(|| "1")
                .unwrap_or("0"),
        ),
    );

    // TODO!@# collision between /vendor/micropython/ports/unix/mpconfigport.h and /projects/unix/mpconfigport.h
    lib.add_public_include(PathBuf::from(mpy_folder).join("ports/unix")); // !@# emu only

    lib.add_include("../rust"); // TODO!@# neeeded rustmods.c

    lib.add_sources([
        "modutime.c",
        "rustmods.c",
        "trezorobj.c",
        "modtrezorapp/modtrezorapp.c",
        "modtrezorconfig/modtrezorconfig.c",
        "modtrezorcrypto/modtrezorcrypto.c",
        "modtrezorcrypto/crc.c",
        "modtrezorio/modtrezorio.c",
        "modtrezorui/modtrezorui.c",
        "modtrezorutils/modtrezorutils.c",
    ]);

    lib.add_sources_from_folder(
        // TODO!@# these files are compiled with -O3 on cortex-m targets
        mpy_folder,
        ["py/gc.c", "py/pystack.c", "py/vm.c"],
    );
    // !@# TODO!@# different sources for cortex-m

    lib.add_sources_from_folder(
        mpy_folder,
        [
            "extmod/modubinascii.c",
            "extmod/moductypes.c",
            "extmod/moduheapq.c",
            "extmod/moduos.c",
            "extmod/modutimeq.c",
            "extmod/utime_mphal.c",
            "shared/readline/readline.c",
            "shared/timeutils/timeutils.c",
            "py/argcheck.c",
            "py/asmarm.c",
            "py/asmbase.c",
            "py/asmthumb.c",
            "py/asmx64.c",
            "py/asmx86.c",
            "py/asmxtensa.c",
            "py/bc.c",
            "py/binary.c",
            "py/builtinevex.c",
            "py/builtinhelp.c",
            "py/builtinimport.c",
            "py/compile.c",
            "py/emitbc.c",
            "py/emitcommon.c",
            "py/emitglue.c",
            "py/emitinlinethumb.c",
            "py/emitinlinextensa.c",
            "py/emitnarm.c",
            "py/emitnative.c",
            "py/emitnthumb.c",
            "py/emitnx64.c",
            "py/emitnx86.c",
            "py/emitnxtensa.c",
            "py/formatfloat.c",
            "py/frozenmod.c",
            "py/lexer.c",
            "py/malloc.c",
            "py/map.c",
            "py/modarray.c",
            "py/modbuiltins.c",
            "py/modcmath.c",
            "py/modcollections.c",
            "py/modgc.c",
            "py/modio.c",
            "py/modmath.c",
            "py/modmicropython.c",
            "py/modstruct.c",
            "py/modsys.c",
            "py/modthread.c",
            "py/moduerrno.c",
            "py/mpprint.c",
            "py/mpstate.c",
            "py/mpz.c",
            "py/nativeglue.c",
            "py/obj.c",
            "py/objarray.c",
            "py/objattrtuple.c",
            "py/objbool.c",
            "py/objboundmeth.c",
            "py/objcell.c",
            "py/objclosure.c",
            "py/objcomplex.c",
            "py/objdeque.c",
            "py/objdict.c",
            "py/objenumerate.c",
            "py/objexcept.c",
            "py/objfilter.c",
            "py/objfloat.c",
            "py/objfun.c",
            "py/objgenerator.c",
            "py/objgetitemiter.c",
            "py/objint.c",
            "py/objint_longlong.c",
            "py/objint_mpz.c",
            "py/objlist.c",
            "py/objmap.c",
            "py/objmodule.c",
            "py/objnamedtuple.c",
            "py/objnone.c",
            "py/objobject.c",
            "py/objpolyiter.c",
            "py/objproperty.c",
            "py/objrange.c",
            "py/objreversed.c",
            "py/objset.c",
            "py/objsingleton.c",
            "py/objslice.c",
            "py/objstr.c",
            "py/objstringio.c",
            "py/objstrunicode.c",
            "py/objtuple.c",
            "py/objtype.c",
            "py/objzip.c",
            "py/opmethods.c",
            "py/parse.c",
            "py/parsenum.c",
            "py/parsenumbase.c",
            "py/persistentcode.c",
            "py/profile.c",
            "py/qstr.c",
            "py/reader.c",
            "py/repl.c",
            "py/runtime.c",
            "py/runtime_utils.c",
            "py/scheduler.c",
            "py/scope.c",
            "py/sequence.c",
            "py/showbc.c",
            "py/smallint.c",
            "py/stackctrl.c",
            "py/stream.c",
            "py/unicode.c",
            "py/vstr.c",
            "py/warning.c",
        ],
    );

    if cfg!(feature = "emulator") {
        lib.add_public_defines([
            ("MICROPY_OOM_CALLBACK", Some("1")),
            ("MP_CONFIGFILE", Some("\"mpconfigport.h\"")),
        ]);

        // This is needed to compile modtrezorutils-meminfo.h that
        // calls STATIC functions in other modules (TODO!@# refactor to avoid this)
        lib.add_define("STATIC", Some(""));

        lib.add_sources_from_folder(
            mpy_folder,
            [
                "py/nlr.c",
                "py/nlraarch64.c",
                "py/nlrsetjmp.c",
                "py/nlrthumb.c",
                "py/nlrx64.c",
                "py/nlrx86.c",
                "ports/unix/alloc.c",
                "ports/unix/gccollect.c",
                "ports/unix/input.c",
                "ports/unix/unix_mphal.c",
                "shared/runtime/gchelper_generic.c",
                "extmod/vfs_posix_file.c",
            ],
        );
    } else if cfg!(feature = "mcu_stm32") {
        lib.add_public_define("MICROPY_OOM_CALLBACK", Some("0"));

        lib.add_sources_from_folder(mpy_folder, ["py/nlrthumb.c"]);
    } else {
        unimplemented!()
    }

    // include OUT_DIR for generated headers
    lib.add_public_include(PathBuf::from(env::var("OUT_DIR").unwrap()));

    build_protobuf_headers();
    build_mpversion_header();

    let upydef_files = lib.build_upydef();
    build_collected_headers(&lib, &upydef_files);

    measure_time("upymod library build time", || lib.build());
}

fn build_collected_headers(lib: &cbuild::CLibrary, upydef_files: &[PathBuf]) {
    let manifest_folder = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let mpy_folder = manifest_folder.join("../../vendor/micropython/py");
    let genhdr_folder = PathBuf::from(env::var("OUT_DIR").unwrap()).join("genhdr");

    measure_time("upymod genhdr", || {
        let qstr_output = genhdr_folder.join("qstrdefs.collected.h");
        let mut qstr_cmd = std::process::Command::new("sh");
        qstr_cmd
            .arg("-c")
            .arg("out=\"$1\"; shift; cat \"$@\" | perl -nle 'print \"Q($1)\" while /MP_QSTR_(\\w+)/g' > \"$out\"")
            .arg("sh")
            .arg(&qstr_output)
            .args(upydef_files);

        let upydef_inputs = upydef_files.iter().collect::<Vec<_>>();

        cbuild::run_cmd(&mut qstr_cmd, &upydef_inputs, &[&qstr_output]);

        let moduledefs_output = genhdr_folder.join("moduledefs.collected.h");
        let mut moduledefs_cmd = std::process::Command::new("sh");
        moduledefs_cmd
            .arg("-c")
            .arg("out=\"$1\"; shift; grep '^MP_REGISTER_MODULE' \"$@\" > \"$out\"")
            .arg("sh")
            .arg(&moduledefs_output)
            .args(upydef_files);

        cbuild::run_cmd(&mut moduledefs_cmd, &upydef_inputs, &[moduledefs_output]);

        let qstr_combined_output = genhdr_folder.join("qstrdefs.combined.h");
        let qstr_preprocessed_raw_output = genhdr_folder.join("qstrdefs.preprocessed.raw.h");
        let qstr_preprocessed_output = genhdr_folder.join("qstrdefs.preprocessed.h");

        let qstr_preprocessed_inputs = vec![
            mpy_folder.join("qstrdefs.h"),
            genhdr_folder.join("qstrdefs.protobuf.h"),
            qstr_output.clone(),
            manifest_folder.join("qstrdefsport.h"),
        ];

        let mut output_file = File::create(&qstr_combined_output)
            .expect("Failed to create qstrdefs.combined.h output file");
        for input in &qstr_preprocessed_inputs {
            let file = File::open(input).expect("Failed to open qstr input file");
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line.expect("Failed to read line from qstr input");
                if line.starts_with("Q(") {
                    writeln!(output_file, "\"{}\"", line)
                        .expect("Failed to write quoted line to qstrdefs.combined.h");
                } else {
                    writeln!(output_file, "{}", line)
                        .expect("Failed to write line to qstrdefs.combined.h");
                }
            }
        }

        lib.preprocess_file(&qstr_combined_output, &qstr_preprocessed_raw_output);

        let input_file = File::open(&qstr_preprocessed_raw_output)
            .expect("Failed to open qstrdefs.preprocessed.raw.h");
        let reader = BufReader::new(input_file);

        let mut output_file = File::create(&qstr_preprocessed_output)
            .expect("Failed to create qstrdefs.preprocessed.h");

        for line in reader.lines() {
            let line = line.expect("Failed to read line from qstrdefs.preprocessed.raw.h");
            let processed_line = if line.starts_with("\"Q(") && line.ends_with('"') {
                line[1..line.len() - 1].to_string()
            } else {
                line
            };

            writeln!(output_file, "{}", processed_line)
                .expect("Failed to write line to qstrdefs.preprocessed.h");
        }

        let qstr_generated_output = genhdr_folder.join("qstrdefs.generated.h");
        let makeqstrdata = mpy_folder.join("makeqstrdata.py");
        let qstr_generated_file =
            File::create(&qstr_generated_output).expect("Failed to create qstrdefs.generated.h");

        let mut qstr_generated_cmd = std::process::Command::new("python3");
        qstr_generated_cmd
            .arg(&makeqstrdata)
            .arg(&qstr_preprocessed_output)
            .stdout(qstr_generated_file);

        let qstr_generated_inputs = vec![makeqstrdata, qstr_preprocessed_output];

        cbuild::run_cmd(
            &mut qstr_generated_cmd,
            &qstr_generated_inputs,
            &[qstr_generated_output],
        );

        // Next step

        //
        // build genhdr/compressed.collected fro upydef files
        // action="cat $SOURCES | sed -nr 's/.*MP_COMPRESSED_ROM_TEXT\\(\\\"(.*)\\\"\\).*/\\1/p' > $TARGET"
    });
}

fn build_mpversion_header() {
    let crate_folder = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let mpy_folder = crate_folder.join("../../vendor/micropython/py");
    let genhdr_folder = PathBuf::from(env::var("OUT_DIR").unwrap()).join("genhdr");

    let tool = mpy_folder.join("makeversionhdr.py");
    let output = genhdr_folder.join("mpversion.h");

    let mut cmd = std::process::Command::new("python3");
    cmd.arg(&tool).arg(&output);

    cbuild::run_cmd(&mut cmd, &[tool], &[output]);
}

fn build_protobuf_headers() {
    let crate_folder = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let protob_folder = crate_folder.join("../../../common/protob");
    let genhdr_folder = PathBuf::from(env::var("OUT_DIR").unwrap()).join("genhdr");

    let mut inputs = cbuild::InputFiles::new();

    inputs
        .add_files_from_folder(&protob_folder, "proto")
        .expect("Failed to collect protobuf sources");

    //TODO!@# include in bootloader??
    inputs.exclude_file("messages-bootloader.proto");

    if false {
        // TODO!@# *cfg!(not(feature = "thp"))*/
        inputs.exclude_file("messages-thp.proto");
    }

    if false {
        // TODO /*cfg!(feature = "pyopt")(// TODO!@#)*/
        inputs.exclude_file("messages-debug.proto");
    }

    let output_file = genhdr_folder.join("qstrdefs.protobuf.h");

    let pb2py_path = protob_folder.join("pb2py");

    let mut cmd = std::process::Command::new(&pb2py_path);
    cmd.args(inputs.as_paths())
        .arg("--qstr-out")
        .arg(&output_file)
        .arg("--bitcoin-only=1"); // TODO!@# use actual bitcoin_only

    cbuild::run_cmd(&mut cmd, &inputs.as_path_refs(), &[output_file]);
}

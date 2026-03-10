fn main() {
    let mut lib = cbuild::CLibrary::new();

    lib.use_lib("io");

    lib.add_includes([".", "protob"]);

    lib.add_sources([
        "bootui.c",
        "main.c",
        "messages.c",
        "version_check.c",
        "protob/protob.c",
        "protob/pb/messages.pb.c",
    ]);

    if !cfg!(feature = "boot_ucb") && !cfg!(feature = "emulator") {
        lib.add_source("header.S");
    }

    if cfg!(feature = "emulator") {
        lib.add_source("emulator.c");
    }

    // nanopb library
    lib.add_public_include("../../../vendor/nanopb");
    lib.add_sources_from_folder(
        "../../../vendor/nanopb/",
        ["pb_common.c", "pb_decode.c", "pb_encode.c"],
    );

    lib.build();

    cbuild::emit_linker_args("bootloader");

    //TODO!@# move to emit_linker_args
    println!("cargo:rustc-link-arg=-Wl,-Bstatic");
    println!("cargo:rustc-link-arg=-lio");
    println!("cargo:rustc-link-arg=-lsec");
    println!("cargo:rustc-link-arg=-lsys");
    println!("cargo:rustc-link-arg=-lrtl");
}

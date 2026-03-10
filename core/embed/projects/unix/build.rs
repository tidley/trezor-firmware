fn main() {
    let mut lib = cbuild::CLibrary::new();

    lib.use_lib("io");
    lib.use_lib("upymod");

    lib.add_includes(["."]);

    lib.add_include("../../rust"); // TODO!@# temporary hack

    lib.add_sources(["main.c", "main_main.c", "profile.c"]);

    lib.build();

    cbuild::emit_linker_args("firmware");

    //TODO!@# move to emit_linker_args
    println!("cargo:rustc-link-arg=-Wl,-Bstatic");
    println!("cargo:rustc-link-arg=-lio");
    println!("cargo:rustc-link-arg=-lsec");
    println!("cargo:rustc-link-arg=-lsys");
    println!("cargo:rustc-link-arg=-lrtl");
}

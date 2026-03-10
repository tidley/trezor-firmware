// Defines sys/rng module
pub fn def_module(lib: &mut cbuild::CLibrary) {
    lib.add_public_include("rng/inc");

    if cfg!(feature = "emulator") {
        lib.add_source("rng/unix/rng.c");
    } else if cfg!(feature = "mcu_stm32") {
        lib.add_source("rng/stm32/rng.c");
    } else {
        unimplemented!();
    }
}

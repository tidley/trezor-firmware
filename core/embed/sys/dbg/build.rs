// Defines sys/dbg module
pub fn def_module(lib: &mut cbuild::CLibrary) {
    lib.add_public_include("dbg/inc");

    lib.add_public_define("USE_DBG_CONSOLE", Some("1"));

    //TODO!@# systemview is missing

    lib.add_sources(["dbg/dbg_console.c", "dbg/syslog.c"]);

    if cfg!(feature = "emulator") {
        lib.add_source("dbg/unix/dbg_console_backend.c");
    } else if cfg!(feature = "mcu_stm32") {
        lib.add_source("stm32/dbg_console_backend.c");
    } else {
        unimplemented!();
    }
}

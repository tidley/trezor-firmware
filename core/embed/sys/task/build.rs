// Defines sys/task module
pub fn def_module(lib: &mut cbuild::CLibrary) {
    lib.add_public_include("task/inc");

    lib.add_sources(["task/applet.c", "task/system.c", "task/sysevent.c"]);

    if cfg!(feature = "emulator") {
        lib.add_sources([
            "task/unix/coreapp.c",
            "task/unix/sdl_event.c",
            "task/unix/systask.c",
            "task/unix/system.c",
        ]);
    } else if cfg!(feature = "mcu_stm32") {
        lib.add_sources([
            "task/stm32/coreapp.c",
            "task/stm32/systask.c",
            "task/stm32/system.c",
        ]);
    } else {
        unimplemented!();
    }
}

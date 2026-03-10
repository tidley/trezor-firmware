// Defines sys/irq module
pub fn def_module(lib: &mut cbuild::CLibrary) {
    lib.add_public_include("irq/inc");

    if cfg!(feature = "emulator") {
        // No implementation
    } else if cfg!(feature = "mcu_stm32") {
        lib.add_source("irq/stm32/irq.c");
    } else {
        unimplemented!();
    }
}

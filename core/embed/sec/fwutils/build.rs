// Defines sec/fwutils module
pub fn def_module(lib: &mut cbuild::CLibrary) {
    lib.add_public_include("fwutils/inc");

    lib.add_source("fwutils/fwutils.c");
}

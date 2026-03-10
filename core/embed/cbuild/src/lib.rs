mod cmd;
mod compile;
mod link;

pub use cmd::InputFiles;
pub use cmd::run_cmd;
pub use compile::CLibrary;
pub use link::emit_linker_args;

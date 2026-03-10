use std::env;
use std::path::PathBuf;

pub fn emit_linker_args(module_type: &str) {
    if has_feature("emulator") {
        println!(
            "cargo:rustc-link-search=/nix/store/rmz7imacazbbf4dqgsb9wwbkh0nx1jkh-SDL2-2.26.4/lib"
        );
        println!(
            "cargo:rustc-link-search=/nix/store/frhqd181g2g6l468g1gzx055dw0y560n-SDL2_image-2.6.3/lib"
        );
        println!("cargo:rustc-link-lib=SDL2");
        println!("cargo:rustc-link-lib=SDL2_image");

        println!("cargo:rustc-link-arg=-Wl,-Bdynamic");
        println!("cargo:rustc-link-lib=c");
        println!("cargo:rustc-link-lib=gcc");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=pthread");
    } else {
        let script = {
            let suffix = if has_feature("secmon_layout") {
                "_secmon"
            } else {
                ""
            };

            if has_feature("model_t3w1") {
                format!("models/T3W1/memory{suffix}.ld")
            } else if has_feature("model_t3t1") {
                format!("models/T3T1/memory{suffix}.ld")
            } else if has_feature("model_t3b1") {
                format!("models/T3B1/memory{suffix}.ld")
            } else if has_feature("model_t2b1") {
                format!("models/T2B1/memory{suffix}.ld")
            } else if has_feature("model_t2t1") {
                format!("models/T2T1/memory{suffix}.ld")
            } else {
                unimplemented!();
            }
        };

        println!("cargo:rustc-link-arg=-T{script}");

        let script = if has_feature("mcu_stm32u5g") {
            format!("sys/linker/stm32u5g/{module_type}.ld")
        } else if has_feature("mcu_stm32u58") {
            format!("sys/linker/stm32u58/{module_type}.ld")
        } else if has_feature("mcu_stm32f4") {
            format!("sys/linker/stm32f4/{module_type}.ld")
        } else {
            unimplemented!();
        };

        println!("cargo:rustc-link-arg=-T{script}");

        let map_file =
            PathBuf::from(env::var("OUT_DIR").unwrap()).join(format!("{module_type}.map"));
        println!("cargo:rustc-link-arg=-Wl,-Map={}", map_file.display());
        println!("cargo:rustc-link-arg=-Wl,--gc-sections");

        println!("cargo:rustc-link-lib=c_nano");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=gcc");
    }
}

fn has_feature(feature: &str) -> bool {
    let env_name = format!("CARGO_FEATURE_{}", feature_to_env_name(feature));
    env::var_os(env_name).is_some()
}

fn feature_to_env_name(feature: &str) -> String {
    feature
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

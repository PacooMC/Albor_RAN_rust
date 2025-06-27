use std::env;
use std::fs;

fn main() {
    // FlexRAN library paths - these would be configured based on actual FlexRAN installation
    let flexran_root = env::var("FLEXRAN_SDK_DIR")
        .unwrap_or_else(|_| "/opt/flexran".to_string());
    
    let flexran_lib_dir = format!("{}/lib", flexran_root);
    let flexran_include_dir = format!("{}/include", flexran_root);
    
    // Check if FlexRAN SDK actually exists
    let flexran_available = fs::metadata(&flexran_lib_dir).is_ok();
    
    if flexran_available {
        // Tell cargo to look for FlexRAN libraries
        println!("cargo:rustc-link-search=native={}", flexran_lib_dir);
        
        // Link FlexRAN libraries
        // These are placeholder names - actual FlexRAN library names would be different
        println!("cargo:rustc-link-lib=flexran_common");
        println!("cargo:rustc-link-lib=flexran_phy");
        
        println!("cargo:rustc-cfg=flexran_available");
    } else {
        // FlexRAN not available, we'll use mock implementation
        println!("cargo:rustc-cfg=flexran_mock");
    }
    
    // Enable AVX512 if feature is enabled
    if cfg!(feature = "avx512") {
        println!("cargo:rustc-cfg=avx512_enabled");
    }
    
    // Compile our C wrapper
    cc::Build::new()
        .file("src/flexran_wrapper.c")
        .include(&flexran_include_dir)
        .flag_if_supported("-mavx512f")
        .flag_if_supported("-mavx512dq")
        .warnings(false) // Disable warnings for placeholder implementation
        .compile("flexran_wrapper");
    
    // Generate bindings (would use bindgen in real implementation)
    // For now, we'll manually define the FFI bindings
    
    println!("cargo:rerun-if-changed=src/flexran_wrapper.c");
    println!("cargo:rerun-if-env-changed=FLEXRAN_SDK_DIR");
}
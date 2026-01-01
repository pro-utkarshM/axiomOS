fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    
    // Set linker script
    println!("cargo:rustc-link-arg=-T{dir}/linker-{arch}.ld");
    println!("cargo:rerun-if-changed={dir}/linker-{arch}.ld");
    
    // Compile architecture-specific assembly files
    match arch.as_str() {
        "riscv64" => {
            println!("cargo:rerun-if-changed=src/arch/riscv64/boot.S");
            
            cc::Build::new()
                .file("src/arch/riscv64/boot.S")
                .flag("-march=rv64gc")
                .flag("-mabi=lp64d")
                .compile("riscv64_boot");
        }
        "aarch64" => {
            println!("cargo:rerun-if-changed=src/arch/aarch64/boot.S");
            println!("cargo:rerun-if-changed=src/arch/aarch64/exception_vectors.S");

            cc::Build::new()
                .file("src/arch/aarch64/boot.S")
                .file("src/arch/aarch64/exception_vectors.S")
                .compile("aarch64_boot");
        }
        "x86_64" => {
            // x86_64 doesn't need assembly compilation (uses Limine)
        }
        _ => {
            println!("cargo:warning=Unknown target architecture: {}", arch);
        }
    }
}

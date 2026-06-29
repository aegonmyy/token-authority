use std::{env, fs, path::PathBuf};

fn main() {
    let out_dir   = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest  = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let elf_src   = manifest.parent().unwrap().join("token_authority.bin");
    let elf_dst   = out_dir.join("token_authority.bin");

    fs::copy(&elf_src, &elf_dst).expect("failed to copy pre-built guest ELF");

    let elf_bytes = fs::read(&elf_src).expect("failed to read ELF");
    let image_id  = risc0_zkvm::compute_image_id(&elf_bytes)
        .expect("failed to compute image ID");

    // risc0 IMAGE_ID constants use as_words() (little-endian u32s).
    let words = image_id.as_words();

    let methods_rs = format!(
        r#"pub const TOKEN_AUTHORITY_ELF: &[u8] = include_bytes!("{elf_dst}");
pub const TOKEN_AUTHORITY_ID: [u32; 8] = {words:?};
"#,
        elf_dst = elf_dst.display(),
        words   = words,
    );

    fs::write(out_dir.join("methods.rs"), methods_rs).unwrap();
    println!("cargo:rerun-if-changed={}", elf_src.display());
}

//! Program signing command.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use colored::Colorize;
use ring::signature::{Ed25519KeyPair, KeyPair};
use sha3::{Digest, Sha3_256};

use crate::signing::{SignedProgramHeader, HEADER_SIZE, MAGIC, VERSION};

/// Sign a BPF program.
pub fn sign_program(input: &str, output: Option<&str>, key_path: &str) -> Result<()> {
    println!("{} {}", "Signing:".cyan(), input);

    // Read the input file
    let program_data =
        fs::read(input).with_context(|| format!("Failed to read input file: {}", input))?;

    // Validate it looks like an ELF file
    if program_data.len() < 4 || &program_data[0..4] != b"\x7fELF" {
        anyhow::bail!("Input file does not appear to be an ELF file");
    }

    // Read the private key
    let key_data =
        fs::read(key_path).with_context(|| format!("Failed to read key file: {}", key_path))?;

    let key_pair = Ed25519KeyPair::from_pkcs8(&key_data)
        .map_err(|_| anyhow::anyhow!("Failed to parse private key"))?;

    // Compute program hash
    let mut hasher = Sha3_256::new();
    hasher.update(&program_data);
    let hash: [u8; 32] = hasher.finalize().into();

    println!("  {} {}", "Hash:".green(), hex_string(&hash[..8]));

    // Sign the hash
    let signature = key_pair.sign(&hash);

    // Get signer ID (first 8 bytes of public key)
    let public_key = key_pair.public_key().as_ref();
    let mut signer_id = [0u8; 8];
    signer_id.copy_from_slice(&public_key[..8]);

    println!("  {} {}", "Signer:".green(), hex_string(&signer_id));

    // Get timestamp
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Create signed program header
    let header = SignedProgramHeader {
        magic: *MAGIC,
        version: VERSION,
        flags: 0,
        reserved: [0; 2],
        program_hash: hash,
        signature: signature.as_ref().try_into().unwrap(),
        signer_id,
        timestamp,
    };

    // Write output file
    let output_path = output.map(|s| s.to_string()).unwrap_or_else(|| {
        let p = Path::new(input);
        let stem = p.file_stem().unwrap().to_string_lossy();
        format!("{}.rbpf", stem)
    });

    let mut output_data = Vec::with_capacity(HEADER_SIZE + program_data.len());
    output_data.extend_from_slice(&header.to_bytes());
    output_data.extend_from_slice(&program_data);

    fs::write(&output_path, &output_data)
        .with_context(|| format!("Failed to write output file: {}", output_path))?;

    println!(
        "\n{} Signed program written to {}",
        "".green(),
        output_path.cyan()
    );
    println!("  {} {} bytes", "Size:".green(), output_data.len());

    Ok(())
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

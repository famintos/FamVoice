use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use serde_json::Value;
use std::{env, fs, io, path::Path, process};

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn decode_wrapper(label: &str, encoded: &str) -> io::Result<String> {
    let bytes = STANDARD
        .decode(encoded.trim())
        .map_err(|error| invalid_data(format!("invalid {label} base64: {error}")))?;
    String::from_utf8(bytes)
        .map_err(|error| invalid_data(format!("invalid UTF-8 in decoded {label}: {error}")))
}

fn verify(artifact_path: &Path, signature_path: &Path, tauri_config_path: &Path) -> io::Result<()> {
    let artifact = fs::read(artifact_path)?;
    let signature_wrapper = fs::read_to_string(signature_path)?;
    let config: Value = serde_json::from_str(&fs::read_to_string(tauri_config_path)?)?;
    let public_key_wrapper = config
        .pointer("/plugins/updater/pubkey")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_data("plugins.updater.pubkey is missing from the Tauri config"))?;

    let public_key_text = decode_wrapper("updater public key", public_key_wrapper)?;
    let signature_text = decode_wrapper("updater signature", &signature_wrapper)?;
    let public_key = PublicKey::decode(&public_key_text)
        .map_err(|error| invalid_data(format!("invalid updater public key: {error:?}")))?;
    let signature = Signature::decode(&signature_text)
        .map_err(|error| invalid_data(format!("invalid updater signature: {error:?}")))?;

    // Match tauri-plugin-updater's verification policy, including legacy signatures.
    public_key
        .verify(&artifact, &signature, true)
        .map_err(|error| {
            invalid_data(format!("updater signature verification failed: {error:?}"))
        })?;

    println!(
        "verified Tauri updater signature ({})",
        signature.trusted_comment()
    );
    Ok(())
}

fn main() {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("usage: verify_updater_signature <artifact> <signature-wrapper> <tauri-config>");
        process::exit(2);
    }

    if let Err(error) = verify(
        Path::new(&args[0]),
        Path::new(&args[1]),
        Path::new(&args[2]),
    ) {
        eprintln!("{error}");
        process::exit(1);
    }
}

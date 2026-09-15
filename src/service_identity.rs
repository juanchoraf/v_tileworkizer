//! A rebuilt GUI must not silently keep an older, incompatible worker alive.
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::{io::Read, sync::OnceLock};

pub fn current() -> Result<&'static str> {
    static ID: OnceLock<String> = OnceLock::new();
    if let Some(id) = ID.get() {
        return Ok(id);
    }
    let mut file = std::fs::File::open(std::env::current_exe()?)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let id: String = digest
        .finalize()
        .iter()
        .map(|b| v_concat::v_concat!("{b:02x}"))
        .collect();
    let _ = ID.set(id);
    Ok(ID.get().expect("binary identity initialized"))
}

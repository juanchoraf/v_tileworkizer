//! Decode manufacturer identity supplied by the monitor, without guessing from its connector.
pub fn pnp(code: &str) -> String {
    match code.to_ascii_uppercase().as_str() {
        "DEL" => "Dell",
        "SAM" | "SEC" => "Samsung",
        "GSM" | "LGD" => "LG",
        "ACR" => "Acer",
        "ACI" | "AUS" => "ASUS",
        "APP" => "Apple",
        "HWP" | "HPN" => "HP",
        "LEN" => "Lenovo",
        "BNQ" => "BenQ",
        "AOC" => "AOC",
        "PHL" => "Philips",
        "VSC" => "ViewSonic",
        "SNY" => "Sony",
        "EIZ" | "ENC" => "EIZO",
        "MSI" => "MSI",
        "NEC" => "NEC",
        "IVM" => "iiyama",
        "HSD" => "HannStar",
        "BOE" => "BOE",
        "AUO" => "AU Optronics",
        "CMN" => "Innolux",
        _ => {
            return if code.len() == 3 && code.bytes().all(|b| b.is_ascii_alphabetic()) {
                code.to_ascii_uppercase()
            } else {
                "Unknown brand".into()
            };
        }
    }
    .into()
}

#[cfg(any(unix, test))]
pub fn vendor(value: u16) -> String {
    let letters = [(value >> 10) & 31, (value >> 5) & 31, value & 31];
    if !letters.iter().all(|v| (1..=26).contains(v)) {
        return "Unknown brand".into();
    }
    let code: String = letters
        .iter()
        .map(|v| (b'A' + *v as u8 - 1) as char)
        .collect();
    pnp(&code)
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn edid(bytes: &[u8]) -> String {
    if bytes.len() < 128
        || bytes[..8] != [0, 255, 255, 255, 255, 255, 255, 0]
        || bytes[..128].iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) != 0
    {
        return "Unknown brand".into();
    }
    vendor(u16::from_be_bytes([bytes[8], bytes[9]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manufacturer_codes_are_decoded_without_guessing() {
        assert_eq!(vendor((4 << 10) | (5 << 5) | 12), "Dell");
        assert_eq!(pnp("sam"), "Samsung");
        assert_eq!(pnp("XYZ"), "XYZ");
        assert_eq!(vendor(0), "Unknown brand");
    }
    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn edid_requires_a_complete_valid_base_block() {
        let mut bytes = [0_u8; 128];
        bytes[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
        bytes[8..10].copy_from_slice(&((4_u16 << 10) | (5 << 5) | 12).to_be_bytes());
        bytes[127] = 0_u8.wrapping_sub(bytes.iter().fold(0_u8, |a, b| a.wrapping_add(*b)));
        assert_eq!(edid(&bytes), "Dell");
        bytes[20] ^= 1;
        assert_eq!(edid(&bytes), "Unknown brand");
        assert_eq!(edid(&bytes[..12]), "Unknown brand");
    }
}

use sha2::{Digest, Sha256};

/// SHA256 hex of an ordered concatenation of fields. `None` values render as empty strings.
///
/// Mirrors the python `generate_hash_id` in `common_cli`/route DDLs: the same field order
/// must produce the same hash on both languages so business keys stay stable.
pub fn generate_hash_id(fields: &[Option<&str>]) -> String {
    let composite: String = fields.iter().map(|f| f.unwrap_or("")).collect();
    format!("{:x}", Sha256::digest(composite.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_python_for_basic_inputs() {
        // Python: hashlib.sha256("abc".encode()).hexdigest()
        let h = generate_hash_id(&[Some("a"), Some("b"), Some("c")]);
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn nones_render_as_empty_strings() {
        let with_nones = generate_hash_id(&[Some("a"), None, Some("b"), None]);
        let with_empties = generate_hash_id(&[Some("a"), Some(""), Some("b"), Some("")]);
        assert_eq!(with_nones, with_empties);
    }

    #[test]
    fn order_matters() {
        let a = generate_hash_id(&[Some("foo"), Some("bar")]);
        let b = generate_hash_id(&[Some("bar"), Some("foo")]);
        assert_ne!(a, b);
    }

    #[test]
    fn produces_64_hex_chars() {
        let h = generate_hash_id(&[Some("anything")]);
        assert_eq!(h.len(), 64);
        assert!(
            h.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }
}

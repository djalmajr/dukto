pub mod cert;
pub mod noise;

use ring::{digest, hkdf};

pub(crate) const SHA256_BYTES: usize = 32;

struct OutputKeyLength;

impl hkdf::KeyType for OutputKeyLength {
    fn len(&self) -> usize {
        SHA256_BYTES
    }
}

pub(crate) fn sha256(data: &[u8]) -> [u8; SHA256_BYTES] {
    let digest = digest::digest(&digest::SHA256, data);
    digest
        .as_ref()
        .try_into()
        .expect("SHA-256 output length is fixed")
}

pub(crate) fn hkdf_sha256(
    input_key_material: &[u8],
    salt: &[u8],
    label: &[u8],
) -> Result<[u8; SHA256_BYTES], ()> {
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, salt);
    let pseudorandom_key = salt.extract(input_key_material);
    let info = [label];
    let output_key_material = pseudorandom_key
        .expand(&info, OutputKeyLength)
        .map_err(|_| ())?;
    let mut output = [0_u8; SHA256_BYTES];
    output_key_material.fill(&mut output).map_err(|_| ())?;
    Ok(output)
}

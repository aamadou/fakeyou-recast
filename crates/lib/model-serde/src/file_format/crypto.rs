use magic_crypt::generic_array::typenum::U256;
use magic_crypt::{MagicCrypt256, MagicCryptError, MagicCryptTrait, new_magic_crypt};
use std::io::Cursor;

/// Handle model encryption and decryption with a single string secret.
pub struct Crypto {
    crypt: MagicCrypt256,
}

impl Crypto {
    pub fn new(crypto_secret: &str) -> Self {
        Self {
            crypt: new_magic_crypt!(crypto_secret, 256),
        }
    }

    pub fn encrypt_bytes(&self, unencrypted_bytes: &[u8]) -> Result<Vec<u8>, MagicCryptError> {
        let mut reader = Cursor::new(unencrypted_bytes);
        let mut encrypted_bytes = Vec::new();
        self.crypt.encrypt_reader_to_writer2::<U256>(&mut reader, &mut encrypted_bytes)?;
        Ok(encrypted_bytes)
    }

    pub fn decrypt_bytes(&self, encrypted_bytes: &[u8]) -> Result<Vec<u8>, MagicCryptError> {
        let decrypted_bytes = self.crypt.decrypt_bytes_to_bytes(&encrypted_bytes)?;
        Ok(decrypted_bytes)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::file_format::crypto::Crypto;

    #[test]
    fn test_encrypt() {
        let crypto = Crypto::new("secret");
        let encrypted_bytes = crypto.encrypt_bytes("message".as_bytes()).unwrap();
        assert_eq!(encrypted_bytes, vec![117, 66, 28, 62, 231, 228, 111, 64, 180, 213, 160, 182, 100, 16, 38, 97]);
    }

    #[test]
    fn test_decrypt() {
        let crypto = Crypto::new("secret");
        let encrypted_bytes = vec![117, 66, 28, 62, 231, 228, 111, 64, 180, 213, 160, 182, 100, 16, 38, 97];
        let decrypted_bytes = crypto.decrypt_bytes(&encrypted_bytes).unwrap();
        assert_eq!(decrypted_bytes, "message".as_bytes());
    }

    #[test]
    fn test_different_secrets() {
        let message = "message";

        let encrypted_1 = Crypto::new("secret_1")
            .encrypt_bytes(message.as_bytes())
            .unwrap();

        let encrypted_2 = Crypto::new("secret_2")
            .encrypt_bytes(message.as_bytes())
            .unwrap();

        assert_ne!(encrypted_1, encrypted_2);
    }

    #[test]
    fn test_same_secrets() {
        let secret = "same_secret";
        let message = "message";

        let encrypted_1 = Crypto::new(secret)
            .encrypt_bytes(message.as_bytes())
            .unwrap();

        let encrypted_2 = Crypto::new(secret)
            .encrypt_bytes(message.as_bytes())
            .unwrap();

        assert_eq!(encrypted_1, encrypted_2);
    }

    #[test]
    fn test_round_trip() {
        let message = "hidden message";
        let crypto = Crypto::new("SUPER SECRET");

        let encrypted_bytes = crypto
            .encrypt_bytes(message.as_bytes())
            .unwrap();

        let decrypted_bytes = crypto
            .decrypt_bytes(&encrypted_bytes)
            .unwrap();

        assert_eq!(message.as_bytes(), decrypted_bytes);
    }
}
use dcl_auth::Wallet;

pub fn random_wallet() -> Wallet {
    loop {
        let bytes: [u8; 32] = rand::random();
        if let Ok(w) = Wallet::from_hex(&hex::encode(bytes)) {
            return w;
        }
    }
}

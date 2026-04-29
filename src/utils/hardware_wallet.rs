use anyhow::Result;
use clap::ValueEnum;

/// Stellar SLIP-0010 / BIP-44 HD derivation path.
/// Default: m/44'/148'/0' (account index 0).
pub const STELLAR_HD_PATH: &str = "m/44'/148'/0'";

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum HardwareWalletKind {
    Ledger,
    Trezor,
}

impl std::fmt::Display for HardwareWalletKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HardwareWalletKind::Ledger => write!(f, "Ledger"),
            HardwareWalletKind::Trezor => write!(f, "Trezor"),
        }
    }
}

/// Basic information returned by a connected hardware wallet.
#[derive(Debug, Clone)]
pub struct HardwareWalletInfo {
    #[allow(dead_code)]
    pub kind: HardwareWalletKind,
    pub device_count: usize,
    #[allow(dead_code)]
    pub stellar_address: Option<String>,
    pub hd_path: String,
}

// ── Feature-disabled stubs ────────────────────────────────────────────────────

#[cfg(not(feature = "hardware-wallet"))]
pub fn connect(kind: HardwareWalletKind) -> Result<HardwareWalletInfo> {
    anyhow::bail!(
        "Hardware wallet support is disabled in this build.\n\
         Rebuild with `cargo build --features hardware-wallet` to enable {} detection.",
        kind
    )
}

#[cfg(not(feature = "hardware-wallet"))]
pub fn sign(_kind: HardwareWalletKind, _message: &[u8]) -> Result<Vec<u8>> {
    anyhow::bail!("Hardware wallet support is disabled in this build.")
}

#[cfg(not(feature = "hardware-wallet"))]
pub fn get_stellar_address(_kind: HardwareWalletKind, _hd_path: &str) -> Result<String> {
    anyhow::bail!("Hardware wallet support is disabled in this build.")
}

#[cfg(not(feature = "hardware-wallet"))]
pub fn device_status(_kind: HardwareWalletKind) -> Result<String> {
    anyhow::bail!("Hardware wallet support is disabled in this build.")
}

// ── Feature-enabled implementations ──────────────────────────────────────────

#[cfg(feature = "hardware-wallet")]
pub fn connect(kind: HardwareWalletKind) -> Result<HardwareWalletInfo> {
    let api = hidapi::HidApi::new()
        .map_err(|e| anyhow::anyhow!("Failed to initialize HID API: {}", e))?;

    let devices: Vec<_> = api.device_list().collect();
    if devices.is_empty() {
        anyhow::bail!(
            "No HID devices detected. Ensure your {} is connected, unlocked, and has the Stellar app open.",
            kind
        );
    }

    Ok(HardwareWalletInfo {
        kind,
        device_count: devices.len(),
        stellar_address: None, // populated lazily via get_stellar_address()
        hd_path: STELLAR_HD_PATH.to_string(),
    })
}

/// Derive the Stellar public key at the given HD path from the hardware wallet.
///
/// The APDU exchange with the Ledger Stellar app (INS 0x02 — GET PUBLIC KEY)
/// is outlined in the Ledger Stellar app documentation:
/// <https://github.com/LedgerHQ/app-stellar/blob/master/doc/APDU.md>
///
/// This function returns a stub address for the initial wiring; a full APDU
/// implementation can replace the inner block without changing the signature.
#[cfg(feature = "hardware-wallet")]
pub fn get_stellar_address(kind: HardwareWalletKind, hd_path: &str) -> Result<String> {
    // Verify the HID subsystem is reachable before claiming success.
    let api = hidapi::HidApi::new()
        .map_err(|e| anyhow::anyhow!("Failed to initialize HID API: {}", e))?;

    let devices: Vec<_> = api.device_list().collect();
    if devices.is_empty() {
        anyhow::bail!("No {} device found. Connect and unlock the device.", kind);
    }

    // Stub: real implementation would open the device, send the GET_PUBLIC_KEY APDU,
    // and decode the 32-byte ed25519 public key from the response.
    //
    // APDU for Ledger Stellar app (INS=0x02, display=false):
    //   CLA=0xE0 INS=0x02 P1=0x00 P2=0x00 Data=<bip32_path_bytes>
    //
    // For now we return a deterministic placeholder so the CLI integration is
    // testable end-to-end without a physical device.
    let placeholder = format!(
        "GHARDWARE{:0>47}",
        hd_path.chars().filter(|c| c.is_ascii_alphanumeric()).count()
    );
    eprintln!(
        "  [hardware-wallet] Note: returning stub address for {} at {}.\n  \
         Replace get_stellar_address() with full APDU exchange for production use.",
        kind, hd_path
    );
    Ok(placeholder)
}

/// Return a human-readable status string for the connected device.
#[cfg(feature = "hardware-wallet")]
pub fn device_status(kind: HardwareWalletKind) -> Result<String> {
    let api = hidapi::HidApi::new()
        .map_err(|e| anyhow::anyhow!("Failed to initialize HID API: {}", e))?;

    let count = api.device_list().count();
    if count == 0 {
        return Ok(format!("{}: not connected", kind));
    }
    Ok(format!("{}: {} HID device(s) visible — ensure Stellar app is open", kind, count))
}

/// Sign raw bytes via the hardware wallet (APDU INS 0x04 — SIGN TRANSACTION).
///
/// Returns the raw 64-byte ed25519 signature.
#[cfg(feature = "hardware-wallet")]
pub fn sign(kind: HardwareWalletKind, _message: &[u8]) -> Result<Vec<u8>> {
    // Verify device is reachable first.
    let api = hidapi::HidApi::new()
        .map_err(|e| anyhow::anyhow!("Failed to initialize HID API: {}", e))?;
    if api.device_list().count() == 0 {
        anyhow::bail!("No {} device found.", kind);
    }
    // Stub: real implementation sends the SIGN APDU and reads back 64 bytes.
    anyhow::bail!(
        "Hardware wallet signing via APDU is not yet implemented for {}.\n\
         Connect your device and use the Stellar app to sign manually, or contribute the APDU flow.",
        kind
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hd_path_constant_is_valid() {
        assert_eq!(STELLAR_HD_PATH, "m/44'/148'/0'");
    }

    #[test]
    fn kind_display() {
        assert_eq!(HardwareWalletKind::Ledger.to_string(), "Ledger");
        assert_eq!(HardwareWalletKind::Trezor.to_string(), "Trezor");
    }
}

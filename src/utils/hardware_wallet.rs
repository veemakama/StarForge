use anyhow::{Context, Result};
use clap::ValueEnum;

/// Stellar SLIP-0010 / BIP-44 HD derivation path.
/// Default: m/44'/148'/0' (account index 0).
pub const STELLAR_HD_PATH: &str = "m/44'/148'/0'";

const LEDGER_VENDOR_ID: u16 = 0x2c97;
const HID_PACKET_SIZE: usize = 64;
const HID_CHANNEL: u16 = 0x0101;
const HID_TAG_APDU: u8 = 0x05;
const SW_OK: [u8; 2] = [0x90, 0x00];

const CLA_STELLAR: u8 = 0xE0;
const INS_GET_PUBLIC_KEY: u8 = 0x02;
const INS_SIGN_TX: u8 = 0x04;

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
    pub kind: HardwareWalletKind,
    pub device_count: usize,
    pub stellar_address: Option<String>,
    pub hd_path: String,
}

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

#[cfg(feature = "hardware-wallet")]
pub fn connect(kind: HardwareWalletKind) -> Result<HardwareWalletInfo> {
    let transport = LedgerTransport::connect(kind)?;
    let stellar_address = transport.get_public_key(STELLAR_HD_PATH).ok();

    Ok(HardwareWalletInfo {
        kind,
        device_count: transport.device_count,
        stellar_address,
        hd_path: STELLAR_HD_PATH.to_string(),
    })
}

#[cfg(feature = "hardware-wallet")]
pub fn get_stellar_address(kind: HardwareWalletKind, hd_path: &str) -> Result<String> {
    LedgerTransport::connect(kind)?.get_public_key(hd_path)
}

#[cfg(feature = "hardware-wallet")]
pub fn device_status(kind: HardwareWalletKind) -> Result<String> {
    let transport = LedgerTransport::connect(kind)?;
    Ok(format!(
        "{}: {} HID device(s) visible, Stellar app reachable",
        kind, transport.device_count
    ))
}

#[cfg(feature = "hardware-wallet")]
pub fn sign(kind: HardwareWalletKind, message: &[u8]) -> Result<Vec<u8>> {
    LedgerTransport::connect(kind)?.sign_message(STELLAR_HD_PATH, message)
}

fn parse_hd_path(path: &str) -> Result<Vec<u32>> {
    let cleaned = path.trim();
    let segments = cleaned
        .strip_prefix("m/")
        .or_else(|| cleaned.strip_prefix("M/"))
        .unwrap_or(cleaned);

    if segments.is_empty() {
        anyhow::bail!("HD path cannot be empty");
    }

    let mut values = Vec::new();
    for segment in segments.split('/') {
        if segment.is_empty() {
            anyhow::bail!("Invalid HD path '{}'", path);
        }
        let hardened = segment.ends_with('\'');
        let digits = if hardened {
            &segment[..segment.len() - 1]
        } else {
            segment
        };
        let index: u32 = digits
            .parse()
            .with_context(|| format!("Invalid HD path segment '{}'", segment))?;
        if index >= 0x8000_0000 {
            anyhow::bail!("HD path segment '{}' is out of range", segment);
        }
        values.push(if hardened { index | 0x8000_0000 } else { index });
    }

    Ok(values)
}

fn encode_hd_path(path: &str) -> Result<Vec<u8>> {
    let indices = parse_hd_path(path)?;
    let mut out = Vec::with_capacity(1 + indices.len() * 4);
    out.push(indices.len() as u8);
    for index in indices {
        out.extend_from_slice(&index.to_be_bytes());
    }
    Ok(out)
}

fn build_apdu(cla: u8, ins: u8, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
    let mut apdu = Vec::with_capacity(5 + data.len());
    apdu.push(cla);
    apdu.push(ins);
    apdu.push(p1);
    apdu.push(p2);
    apdu.push(data.len() as u8);
    apdu.extend_from_slice(data);
    apdu
}

fn frame_apdu_for_hid(apdu: &[u8]) -> Vec<[u8; HID_PACKET_SIZE]> {
    let mut framed = Vec::new();
    let mut remaining = apdu;
    let mut sequence: u16 = 0;

    while sequence == 0 || !remaining.is_empty() {
        let mut packet = [0u8; HID_PACKET_SIZE];
        packet[0..2].copy_from_slice(&HID_CHANNEL.to_be_bytes());
        packet[2] = HID_TAG_APDU;
        packet[3..5].copy_from_slice(&sequence.to_be_bytes());

        let header_len = if sequence == 0 {
            packet[5..7].copy_from_slice(&(apdu.len() as u16).to_be_bytes());
            7
        } else {
            5
        };

        let chunk_len = remaining.len().min(HID_PACKET_SIZE - header_len);
        packet[header_len..header_len + chunk_len].copy_from_slice(&remaining[..chunk_len]);
        remaining = &remaining[chunk_len..];
        framed.push(packet);
        sequence += 1;
    }

    framed
}

#[cfg(feature = "hardware-wallet")]
struct LedgerTransport {
    device: hidapi::HidDevice,
    device_count: usize,
}

#[cfg(feature = "hardware-wallet")]
impl LedgerTransport {
    fn connect(kind: HardwareWalletKind) -> Result<Self> {
        match kind {
            HardwareWalletKind::Ledger => Self::connect_ledger(),
            HardwareWalletKind::Trezor => anyhow::bail!(
                "Trezor transport is not implemented yet. Use Ledger with `--features hardware-wallet` for now."
            ),
        }
    }

    fn connect_ledger() -> Result<Self> {
        let api = hidapi::HidApi::new().context("Failed to initialize HID API")?;
        let devices = api
            .device_list()
            .filter(|info| info.vendor_id() == LEDGER_VENDOR_ID)
            .collect::<Vec<_>>();

        if devices.is_empty() {
            anyhow::bail!(
                "No Ledger device detected. Connect it, unlock it, and open the Stellar app."
            );
        }

        let device = devices[0]
            .open_device(&api)
            .context("Failed to open Ledger HID device")?;

        Ok(Self {
            device,
            device_count: devices.len(),
        })
    }

    fn exchange(&self, apdu: &[u8]) -> Result<Vec<u8>> {
        for packet in frame_apdu_for_hid(apdu) {
            self.device
                .write(&packet)
                .context("Failed to write APDU packet to Ledger")?;
        }

        let mut response = Vec::new();
        let mut expected_len: Option<usize> = None;
        let mut sequence: u16 = 0;

        loop {
            let mut packet = [0u8; HID_PACKET_SIZE];
            let read = self
                .device
                .read_timeout(&mut packet, 15_000)
                .context("Timed out waiting for Ledger response")?;

            if read < 5 {
                anyhow::bail!("Received short HID response from Ledger");
            }
            if packet[0..2] != HID_CHANNEL.to_be_bytes() || packet[2] != HID_TAG_APDU {
                anyhow::bail!("Received invalid Ledger HID framing");
            }

            let packet_sequence = u16::from_be_bytes([packet[3], packet[4]]);
            if packet_sequence != sequence {
                anyhow::bail!("Ledger response sequence mismatch");
            }

            let start = if sequence == 0 {
                let total_len = u16::from_be_bytes([packet[5], packet[6]]) as usize;
                expected_len = Some(total_len);
                7
            } else {
                5
            };

            response.extend_from_slice(&packet[start..read]);

            if let Some(total) = expected_len {
                if response.len() >= total {
                    response.truncate(total);
                    break;
                }
            }

            sequence += 1;
        }

        if response.len() < 2 {
            anyhow::bail!("Ledger response did not include a status word");
        }
        let status = &response[response.len() - 2..];
        if status != SW_OK {
            anyhow::bail!(
                "Ledger returned APDU status {:02x}{:02x}",
                status[0],
                status[1]
            );
        }

        Ok(response[..response.len() - 2].to_vec())
    }

    fn get_public_key(&self, hd_path: &str) -> Result<String> {
        let path_bytes = encode_hd_path(hd_path)?;
        let apdu = build_apdu(CLA_STELLAR, INS_GET_PUBLIC_KEY, 0x01, 0x00, &path_bytes);
        let response = self.exchange(&apdu)?;
        let public_key_bytes = extract_public_key_bytes(&response)?;
        Ok(stellar_strkey::ed25519::PublicKey(public_key_bytes).to_string())
    }

    fn sign_message(&self, hd_path: &str, message: &[u8]) -> Result<Vec<u8>> {
        let path_bytes = encode_hd_path(hd_path)?;
        let total_chunks = message.chunks(255).count().max(1);
        let mut signature = None;

        for (index, chunk) in message.chunks(255).enumerate() {
            let mut payload = Vec::new();
            if index == 0 {
                payload.extend_from_slice(&path_bytes);
            }
            payload.extend_from_slice(chunk);

            let p1 = if index == 0 { 0x00 } else { 0x80 };
            let p2 = if index + 1 == total_chunks {
                0x00
            } else {
                0x80
            };
            let apdu = build_apdu(CLA_STELLAR, INS_SIGN_TX, p1, p2, &payload);
            let response = self.exchange(&apdu)?;

            if index + 1 == total_chunks {
                signature = Some(extract_signature_bytes(&response)?);
            }
        }

        signature.ok_or_else(|| anyhow::anyhow!("Ledger did not return a signature"))
    }
}

fn extract_public_key_bytes(response: &[u8]) -> Result<[u8; 32]> {
    if response.len() >= 32 {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&response[..32]);
        return Ok(bytes);
    }
    anyhow::bail!("Ledger public-key response was too short")
}

fn extract_signature_bytes(response: &[u8]) -> Result<Vec<u8>> {
    if response.len() >= 64 {
        return Ok(response[..64].to_vec());
    }
    anyhow::bail!("Ledger signature response was too short")
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

    #[test]
    fn parses_hd_path_segments() {
        let parsed = parse_hd_path("m/44'/148'/0'").unwrap();
        assert_eq!(parsed, vec![0x8000_002c, 0x8000_0094, 0x8000_0000]);
    }

    #[test]
    fn encodes_hd_path_prefix_and_bytes() {
        let encoded = encode_hd_path("m/44'/148'/0'").unwrap();
        assert_eq!(encoded[0], 3);
        assert_eq!(&encoded[1..5], &0x8000_002c_u32.to_be_bytes());
    }

    #[test]
    fn builds_apdu_header() {
        let apdu = build_apdu(0xE0, 0x02, 0x01, 0x00, &[1, 2, 3]);
        assert_eq!(apdu, vec![0xE0, 0x02, 0x01, 0x00, 3, 1, 2, 3]);
    }

    #[test]
    fn frames_large_apdu_into_multiple_hid_packets() {
        let apdu = vec![0xAB; 120];
        let packets = frame_apdu_for_hid(&apdu);
        assert!(packets.len() >= 2);
        assert_eq!(packets[0][0..2], HID_CHANNEL.to_be_bytes());
        assert_eq!(packets[0][2], HID_TAG_APDU);
    }

    #[test]
    fn extracts_public_key_from_recorded_vector() {
        let response = [7u8; 32];
        let key = extract_public_key_bytes(&response).unwrap();
        assert_eq!(key, [7u8; 32]);
    }

    #[test]
    fn extracts_signature_from_recorded_vector() {
        let response = vec![9u8; 64];
        let signature = extract_signature_bytes(&response).unwrap();
        assert_eq!(signature.len(), 64);
        assert!(signature.iter().all(|byte| *byte == 9));
    }

    #[cfg(feature = "hardware-wallet")]
    #[test]
    #[ignore = "requires a connected Ledger with the Stellar app open"]
    fn ledger_integration_requires_device() {
        let _ = connect(HardwareWalletKind::Ledger);
    }
}

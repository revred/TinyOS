//! The signed console manifest.
//!
//! The manifest enumerates exactly the console's verbs for exactly one webview label. It is
//! signed (ed25519) at rest; the running console holds only the public key. Verification is
//! the *only* way to obtain a [`VerifiedManifest`], which is the only thing
//! [`crate::authority::ConsoleAuthority`] accepts — so an unsigned or tampered manifest
//! cannot reach the resolver at all, rather than reaching it and being trusted.

use std::collections::BTreeSet;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// The signed portion of the manifest. Field order is the canonical byte order:
/// the signature covers `serde_json::to_vec` of this struct, and struct fields serialize
/// in declaration order, so the bytes are deterministic for a fixed crate version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestPayload {
    /// The one webview label this manifest grants to. Runtime-derived on the resolver side.
    pub console: String,
    /// The verbs — invoke commands — the console may use. Exactly these, nothing else.
    pub verbs: Vec<String>,
    /// The enumerated tab webview labels (17G): a fixed list, not a namespace. A label
    /// not listed here — the reserved region included — holds no verbs at all.
    #[serde(default)]
    pub tab_labels: Vec<String>,
    /// The verbs any enumerated tab identity may use. Disjoint by intent from the
    /// console's chrome verbs: a tab cannot open tabs, the chrome cannot run lines.
    #[serde(default)]
    pub tab_verbs: Vec<String>,
}

/// The manifest as it exists at rest: payload plus detached signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    /// What the signature covers.
    pub payload: ManifestPayload,
    /// ed25519 signature over the canonical payload bytes, lowercase hex.
    pub signature_hex: String,
}

/// Everything that can make verification fail. One variant per distinct cause so a test can
/// assert the *reason*, not just the refusal.
#[derive(Debug, PartialEq, Eq)]
pub enum ManifestError {
    /// The public key hex string did not decode to a valid ed25519 verifying key.
    BadPublicKey,
    /// The signature hex did not decode to a well-formed signature.
    BadSignatureEncoding,
    /// The signature is well-formed but does not verify over the payload bytes.
    SignatureMismatch,
    /// The manifest JSON itself could not be parsed.
    BadManifestJson,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            ManifestError::BadPublicKey => "public key hex is not a valid ed25519 key",
            ManifestError::BadSignatureEncoding => "signature hex is not a well-formed signature",
            ManifestError::SignatureMismatch => "signature does not verify over the payload",
            ManifestError::BadManifestJson => "manifest JSON does not parse",
        };
        f.write_str(text)
    }
}

/// A manifest that has passed signature verification. No public constructor: the only way
/// in is [`SignedManifest::verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedManifest {
    console: String,
    verbs: BTreeSet<String>,
    tab_labels: BTreeSet<String>,
    tab_verbs: BTreeSet<String>,
}

impl VerifiedManifest {
    /// The webview label the manifest grants to.
    pub fn console(&self) -> &str {
        &self.console
    }

    /// Whether `verb` is enumerated for the console chrome identity.
    pub fn allows(&self, verb: &str) -> bool {
        self.verbs.contains(verb)
    }

    /// Whether `label` is an enumerated tab identity.
    pub fn is_tab(&self, label: &str) -> bool {
        self.tab_labels.contains(label)
    }

    /// Whether `verb` is enumerated for tab identities.
    pub fn tab_allows(&self, verb: &str) -> bool {
        self.tab_verbs.contains(verb)
    }

    /// The enumerated chrome verbs, for display.
    pub fn verbs(&self) -> impl Iterator<Item = &str> {
        self.verbs.iter().map(String::as_str)
    }

    /// The enumerated tab verbs, for display.
    pub fn tab_verbs(&self) -> impl Iterator<Item = &str> {
        self.tab_verbs.iter().map(String::as_str)
    }
}

impl SignedManifest {
    /// Parse a manifest from its at-rest JSON.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json).map_err(|_| ManifestError::BadManifestJson)
    }

    /// Sign `payload` with the given secret key (32 bytes, hex). Used by the
    /// `sign-manifest` tool and by tests; the console binary itself never signs.
    pub fn sign(payload: ManifestPayload, secret_key_hex: &str) -> Result<Self, ManifestError> {
        let secret: [u8; 32] = hex::decode(secret_key_hex.trim())
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(ManifestError::BadPublicKey)?;
        let signing_key = SigningKey::from_bytes(&secret);
        let bytes = canonical_bytes(&payload);
        let signature = signing_key.sign(&bytes);
        Ok(SignedManifest { payload, signature_hex: hex::encode(signature.to_bytes()) })
    }

    /// Verify against `public_key_hex` (32 bytes, hex). Success is the only path to a
    /// [`VerifiedManifest`].
    pub fn verify(&self, public_key_hex: &str) -> Result<VerifiedManifest, ManifestError> {
        let key_bytes: [u8; 32] = hex::decode(public_key_hex.trim())
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(ManifestError::BadPublicKey)?;
        let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| ManifestError::BadPublicKey)?;
        let signature_bytes: [u8; 64] = hex::decode(self.signature_hex.trim())
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(ManifestError::BadSignatureEncoding)?;
        let signature = Signature::from_bytes(&signature_bytes);
        key.verify(&canonical_bytes(&self.payload), &signature)
            .map_err(|_| ManifestError::SignatureMismatch)?;
        Ok(VerifiedManifest {
            console: self.payload.console.clone(),
            verbs: self.payload.verbs.iter().cloned().collect(),
            tab_labels: self.payload.tab_labels.iter().cloned().collect(),
            tab_verbs: self.payload.tab_verbs.iter().cloned().collect(),
        })
    }
}

/// Derive the hex public key for a hex secret key — the `sign-manifest` tool's output side.
pub fn public_key_hex(secret_key_hex: &str) -> Result<String, ManifestError> {
    let secret: [u8; 32] = hex::decode(secret_key_hex.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(ManifestError::BadPublicKey)?;
    Ok(hex::encode(SigningKey::from_bytes(&secret).verifying_key().to_bytes()))
}

/// The bytes the signature covers.
fn canonical_bytes(payload: &ManifestPayload) -> Vec<u8> {
    serde_json::to_vec(payload).expect("a ManifestPayload always serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway signing key for tests: any fixed 32 bytes are a valid ed25519 secret.
    const TEST_SECRET: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";

    fn test_payload() -> ManifestPayload {
        ManifestPayload {
            console: "console".into(),
            verbs: vec![
                "launch_fixture".into(),
                "send_line".into(),
                "read_stream".into(),
                "terminate".into(),
            ],
            tab_labels: vec!["tab-1".into(), "tab-2".into()],
            tab_verbs: vec!["run_line".into(), "read_tab".into()],
        }
    }

    /// M1 — sign → verify round-trips, and the verified view answers exactly the verbs.
    #[test]
    fn m1_sign_verify_roundtrip() {
        let signed = SignedManifest::sign(test_payload(), TEST_SECRET).unwrap();
        let public = public_key_hex(TEST_SECRET).unwrap();
        let verified = signed.verify(&public).expect("a freshly signed manifest must verify");
        assert_eq!(verified.console(), "console");
        assert!(verified.allows("launch_fixture"));
        assert!(verified.allows("terminate"));
        assert!(!verified.allows("format_disk"));
    }

    /// M2 — tampering with the payload after signing is caught: a verb added post-hoc does
    /// not verify. This is the whole point of signing the enumeration.
    #[test]
    fn m2_tampered_payload_fails_closed() {
        let mut signed = SignedManifest::sign(test_payload(), TEST_SECRET).unwrap();
        signed.payload.verbs.push("format_disk".into());
        let public = public_key_hex(TEST_SECRET).unwrap();
        assert_eq!(signed.verify(&public), Err(ManifestError::SignatureMismatch));
    }

    /// M3 — a manifest signed by a different key does not verify against ours.
    #[test]
    fn m3_wrong_key_fails_closed() {
        let other_secret = "0000000000000000000000000000000000000000000000000000000000000042";
        let signed = SignedManifest::sign(test_payload(), other_secret).unwrap();
        let public = public_key_hex(TEST_SECRET).unwrap();
        assert_eq!(signed.verify(&public), Err(ManifestError::SignatureMismatch));
    }

    /// M5 — the tab enumeration is part of the signed payload: it round-trips through
    /// verification, and a tab label or tab verb added after signing fails closed —
    /// exactly the property that makes "authority stays manifest-shaped" (17G §1.4) true
    /// for tabs and not just for the chrome.
    #[test]
    fn m5_tab_grants_are_signed() {
        let signed = SignedManifest::sign(test_payload(), TEST_SECRET).unwrap();
        let public = public_key_hex(TEST_SECRET).unwrap();
        let verified = signed.verify(&public).unwrap();
        assert!(verified.is_tab("tab-1") && verified.is_tab("tab-2"));
        assert!(!verified.is_tab("reserved"), "the reserved region is never a tab identity");
        assert!(verified.tab_allows("run_line") && !verified.tab_allows("open_tab"));

        let mut grown_label = signed.clone();
        grown_label.payload.tab_labels.push("tab-666".into());
        assert_eq!(grown_label.verify(&public), Err(ManifestError::SignatureMismatch));
        let mut grown_verb = signed;
        grown_verb.payload.tab_verbs.push("format_disk".into());
        assert_eq!(grown_verb.verify(&public), Err(ManifestError::SignatureMismatch));
    }

    /// M4 — malformed inputs fail closed with the *encoding* error, never a panic and never
    /// a pass: garbage signature hex, garbage key hex, garbage JSON.
    #[test]
    fn m4_malformed_inputs_fail_closed() {
        let signed = SignedManifest::sign(test_payload(), TEST_SECRET).unwrap();
        let public = public_key_hex(TEST_SECRET).unwrap();

        let mut bad_sig = signed.clone();
        bad_sig.signature_hex = "zz-not-hex".into();
        assert_eq!(bad_sig.verify(&public), Err(ManifestError::BadSignatureEncoding));

        assert_eq!(signed.verify("too-short"), Err(ManifestError::BadPublicKey));

        assert_eq!(
            SignedManifest::from_json("{not json").unwrap_err(),
            ManifestError::BadManifestJson
        );
    }
}

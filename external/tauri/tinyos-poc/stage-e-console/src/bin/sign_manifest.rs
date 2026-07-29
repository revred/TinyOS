//! Dev-time signer: reads `manifest/console-payload.json` and
//! `manifest/dev-signing-key.hex`, writes `manifest/console-manifest.json` (payload +
//! signature) and `manifest/console-pubkey.hex` (what the app embeds and trusts).
//!
//! The signing key committed beside the payload is a **PoC development key**, not a custody
//! model: Stage E demonstrates that authority flows from a signed enumeration through the
//! resolver seam, not how signing keys are kept. The report states this as a non-claim.

use std::path::Path;

use stage_e_console::manifest::{public_key_hex, ManifestPayload, SignedManifest};

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
    let payload: ManifestPayload = serde_json::from_str(
        &std::fs::read_to_string(dir.join("console-payload.json"))
            .expect("manifest/console-payload.json must exist"),
    )
    .expect("payload JSON must parse");
    let secret = std::fs::read_to_string(dir.join("dev-signing-key.hex"))
        .expect("manifest/dev-signing-key.hex must exist");

    let signed = SignedManifest::sign(payload, &secret).expect("signing must succeed");
    std::fs::write(
        dir.join("console-manifest.json"),
        serde_json::to_string_pretty(&signed).expect("manifest serializes"),
    )
    .expect("writing console-manifest.json");
    std::fs::write(
        dir.join("console-pubkey.hex"),
        public_key_hex(&secret).expect("public key derives"),
    )
    .expect("writing console-pubkey.hex");
    println!("signed: manifest/console-manifest.json, manifest/console-pubkey.hex");
}

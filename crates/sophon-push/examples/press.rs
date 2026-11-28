//! A phone's press on a notification, for trying the service's actions on
//! one machine: `cargo run -p sophon-push --example press -- <gateway
//! host:port> <computer id> <bridge secret, base64> <pairing token>
//! <agent> <permission> allow|deny`. (A phone has the bridge's public key
//! from its pairing; here it is made from the secret the service has.)

use base64::{Engine, engine::general_purpose::STANDARD as B64};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [gateway, id, secret, token, agent, permission, allow] = &a[..] else {
        eprintln!("press <gateway> <id> <secret b64> <token> <agent> <permission> allow|deny");
        std::process::exit(2);
    };
    let secret: [u8; 32] = B64.decode(secret).expect("base64").try_into().expect("32 bytes");
    let public = *crypto_box::SecretKey::from(secret).public_key().as_bytes();
    let act = sophon_push::Act::Permission { agent: agent.clone(), permission: permission.clone(), allow: allow == "allow" };
    match sophon_push::gateway::deliver(gateway, false, id, &public, token, &act).await {
        Ok(()) => println!("taken"),
        Err(e) => println!("refused: {e}"),
    }
}

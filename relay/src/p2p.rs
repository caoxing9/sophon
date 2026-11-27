//! The relay for peer to peer (iroh's, `iroh-relay`): a computer and its
//! phones find each other through it and punch through their NATs; what
//! cannot go direct goes through it, end-to-end encrypted. With it here a
//! Sophon that names this relay needs no one else's servers.
//!
//! On when a certificate is given (`P2P_TLS_CERT`, `P2P_TLS_KEY`: PEM
//! files; an IP's certificate does, no domain needed): iroh's address
//! discovery is QUIC, which needs TLS. Read again every hour, so a renewed
//! certificate (a short-lived one is renewed every few days) is taken
//! without a restart. The address computers are told (`P2P_RELAY_URL`,
//! e.g. `https://1.2.3.4:3443`) is answered at `/p2p`.
//!
//! The ports, by default: HTTPS 3443 (the relay), HTTP 3340 (the captive
//! portal probe), UDP 7842 (address discovery).

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use iroh_relay::server::{CertConfig, QuicConfig, RelayConfig, Server, ServerConfig, TlsConfig};
use rustls::{
    crypto::CryptoProvider,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
};

/// How often the certificate is read again.
const RELOAD: Duration = Duration::from_secs(60 * 60);

/// Where the relay listens and what it is called, from the environment.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub url: String,
    pub cert: PathBuf,
    pub key: PathBuf,
    pub https: SocketAddr,
    pub http: SocketAddr,
    pub quic: SocketAddr,
}

impl Settings {
    /// `None` when no certificate (or no address to announce) is given:
    /// the relay for peer to peer stays off.
    pub fn from_env(var: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let cert = var("P2P_TLS_CERT").filter(|v| !v.is_empty())?;
        let key = var("P2P_TLS_KEY").filter(|v| !v.is_empty())?;
        let url = var("P2P_RELAY_URL").filter(|v| v.starts_with("https://"))?;
        let addr = |name: &str, default: &str| var(name).filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_owned()).parse().ok();
        Some(Self {
            url: url.trim_end_matches('/').to_owned(),
            cert: cert.into(),
            key: key.into(),
            https: addr("P2P_RELAY_HTTPS", "0.0.0.0:3443")?,
            http: addr("P2P_RELAY_HTTP", "0.0.0.0:3340")?,
            quic: addr("P2P_RELAY_QUIC", "0.0.0.0:7842")?,
        })
    }
}

/// The certificate in use, swapped for the file's as it changes.
#[derive(Debug)]
struct Reloading {
    current: RwLock<Arc<CertifiedKey>>,
    provider: Arc<CryptoProvider>,
    cert: PathBuf,
    key: PathBuf,
}

impl Reloading {
    fn load(cert: &Path, key: &Path, provider: &CryptoProvider) -> Result<Arc<CertifiedKey>, String> {
        let certs = CertificateDer::pem_file_iter(cert)
            .map_err(|e| format!("{}: {e}", cert.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("{}: {e}", cert.display()))?;
        let key = PrivateKeyDer::from_pem_file(key).map_err(|e| format!("{}: {e}", key.display()))?;
        let signing = provider.key_provider.load_private_key(key).map_err(|e| e.to_string())?;
        Ok(Arc::new(CertifiedKey::new(certs, signing)))
    }

    fn reload(&self) {
        match Self::load(&self.cert, &self.key, &self.provider) {
            Ok(fresh) => {
                if let Ok(mut current) = self.current.write() {
                    *current = fresh;
                }
            }
            Err(e) => log::warn!("p2p relay certificate not read again: {e}"),
        }
    }
}

impl ResolvesServerCert for Reloading {
    fn resolve(&self, _: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        self.current.read().ok().map(|c| c.clone())
    }
}

/// Starts the relay; it runs until the server is dropped.
pub async fn start(settings: &Settings) -> Result<Server, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let resolver = Arc::new(Reloading {
        current: RwLock::new(Reloading::load(&settings.cert, &settings.key, &provider)?),
        provider: provider.clone(),
        cert: settings.cert.clone(),
        key: settings.key.clone(),
    });
    {
        let resolver = resolver.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(RELOAD).await;
                resolver.reload();
            }
        });
    }
    let server_config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_cert_resolver(resolver);
    let mut relay = RelayConfig::new(settings.http);
    relay.tls = Some(TlsConfig::new(settings.https, CertConfig::Manual { server_config }));
    let mut config = ServerConfig::default();
    config.relay = Some(relay);
    config.quic = Some(QuicConfig::new(settings.quic));
    Server::spawn(config).await.map_err(|e| e.to_string())
}

/// What `/p2p` answers: the relay computers are to use, or none.
pub fn answer(url: Option<&str>) -> String {
    serde_json::json!({ "url": url }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_on_only_with_a_certificate_and_an_address() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        assert_eq!(Settings::from_env(env(&[])), None);
        assert_eq!(Settings::from_env(env(&[("P2P_TLS_CERT", "/c.pem"), ("P2P_TLS_KEY", "/k.pem")])), None, "no address to announce");
        assert_eq!(
            Settings::from_env(env(&[("P2P_TLS_CERT", "/c.pem"), ("P2P_TLS_KEY", "/k.pem"), ("P2P_RELAY_URL", "http://1.2.3.4:3443")])),
            None,
            "not TLS"
        );
        let on = Settings::from_env(env(&[("P2P_TLS_CERT", "/c.pem"), ("P2P_TLS_KEY", "/k.pem"), ("P2P_RELAY_URL", "https://1.2.3.4:3443/")])).unwrap();
        assert_eq!(on.url, "https://1.2.3.4:3443");
        assert_eq!(on.https, "0.0.0.0:3443".parse().unwrap());
        assert_eq!(on.quic, "0.0.0.0:7842".parse().unwrap());
        assert_eq!(answer(Some(&on.url)), r#"{"url":"https://1.2.3.4:3443"}"#);
        assert_eq!(answer(None), r#"{"url":null}"#);
    }

    /// A certificate for `127.0.0.1`, written to `dir`: its DER.
    fn self_signed(dir: &Path) -> Vec<u8> {
        let rcgen::CertifiedKey { cert, signing_key } = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_owned()]).unwrap();
        std::fs::write(dir.join("cert.pem"), cert.pem()).unwrap();
        std::fs::write(dir.join("key.pem"), signing_key.serialize_pem()).unwrap();
        cert.der().to_vec()
    }

    /// The certificate the server shows over TLS at `addr`.
    async fn shown(addr: SocketAddr) -> Vec<u8> {
        #[derive(Debug)]
        struct Any;
        impl rustls::client::danger::ServerCertVerifier for Any {
            fn verify_server_cert(
                &self,
                _: &CertificateDer<'_>,
                _: &[CertificateDer<'_>],
                _: &rustls::pki_types::ServerName<'_>,
                _: &[u8],
                _: rustls::pki_types::UnixTime,
            ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            }
            fn verify_tls12_signature(
                &self,
                _: &[u8],
                _: &CertificateDer<'_>,
                _: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
                Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
            }
            fn verify_tls13_signature(
                &self,
                _: &[u8],
                _: &CertificateDer<'_>,
                _: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
                Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
            }
            fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
                rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
            }
        }
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Any))
            .with_no_client_auth();
        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        let name = rustls::pki_types::ServerName::try_from("127.0.0.1").unwrap();
        let tls = tokio_rustls::TlsConnector::from(Arc::new(config)).connect(name, tcp).await.unwrap();
        tls.get_ref().1.peer_certificates().unwrap()[0].to_vec()
    }

    /// Started with a certificate, the relay serves HTTPS and QUIC with it;
    /// a renewed certificate is shown once read again, with no restart.
    #[tokio::test]
    async fn it_serves_with_the_certificate_and_takes_a_renewed_one() {
        let dir = std::env::temp_dir().join(format!("sophon-p2p-relay-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = self_signed(&dir);
        let settings = Settings {
            url: "https://127.0.0.1:0".into(),
            cert: dir.join("cert.pem"),
            key: dir.join("key.pem"),
            https: "127.0.0.1:0".parse().unwrap(),
            http: "127.0.0.1:0".parse().unwrap(),
            quic: "127.0.0.1:0".parse().unwrap(),
        };
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let server = start(&settings).await.unwrap();
        let https = server.https_addr().expect("https");
        assert!(server.quic_addr().is_some(), "address discovery on");
        assert_eq!(shown(https).await, first);

        // Renewed: the files change, and the next read takes them.
        let second = self_signed(&dir);
        assert_ne!(first, second);
        let reloading = Reloading {
            current: RwLock::new(Reloading::load(&settings.cert, &settings.key, &provider).unwrap()),
            provider,
            cert: settings.cert.clone(),
            key: settings.key.clone(),
        };
        reloading.reload();
        assert_eq!(reloading.current.read().unwrap().cert[0].to_vec(), second);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

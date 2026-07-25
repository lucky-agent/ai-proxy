use std::path::{Path, PathBuf};
use std::sync::Arc;

use rama::error::{BoxError, ErrorContext};
use rama::tls::rustls::server::{DynamicConfigProvider, RustlsServerConfigExt};
use rama::tls::server::TlsServerConfig;
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::{ClientHello, ServerConfig};

const CA_ORG_NAME: &str = "ai-proxy";
const CA_COMMON_NAME: &str = "ai-proxy";

fn ca_cert_path(dir: &Path) -> PathBuf {
    dir.join("ca-cert.pem")
}
fn ca_key_path(dir: &Path) -> PathBuf {
    dir.join("ca-key.pem")
}

pub(crate) struct MitmCertProvider {
    ca_key_pem: String,
    ca_cert_der: CertificateDer<'static>,
}

impl MitmCertProvider {
    pub(crate) fn try_new(ca_cert_dir: &Path) -> Result<Self, BoxError> {
        let cert_path = ca_cert_path(ca_cert_dir);
        let key_path = ca_key_path(ca_cert_dir);

        if cert_path.exists() && key_path.exists() {
            let ca_key_pem = std::fs::read_to_string(&key_path).context("read CA key PEM")?;
            let ca_cert_pem = std::fs::read_to_string(&cert_path).context("read CA cert PEM")?;
            let ca_cert_der = pem_to_cert_der(&ca_cert_pem);

            log::info!(
                "Loaded existing CA certificate from {}",
                cert_path.display()
            );

            Ok(Self {
                ca_key_pem,
                ca_cert_der,
            })
        } else {
            generate_new_ca(ca_cert_dir)
        }
    }

    pub(crate) fn into_tls_server_config(self) -> TlsServerConfig {
        TlsServerConfig::new().with_dynamic_config(Arc::new(self))
    }
}

impl DynamicConfigProvider for MitmCertProvider {
    async fn get_config(
        &self,
        client_hello: ClientHello<'_>,
    ) -> Result<Arc<ServerConfig>, BoxError> {
        let hostname = client_hello.server_name().unwrap_or("localhost");

        let ca_key_pair = KeyPair::from_pem(&self.ca_key_pem).context("parse CA key PEM")?;
        let ca_issuer = create_ca_issuer(ca_key_pair);

        let (server_cert_der, server_key_der) =
            generate_server_cert_for_hostname(&ca_issuer, hostname)?;

        let cert_chain = vec![server_cert_der, self.ca_cert_der.clone()];

        let mut config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, server_key_der)
            .map_err(|e| Box::new(e) as BoxError)?;
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        Ok(Arc::new(config))
    }
}

fn generate_new_ca(dir: &Path) -> Result<MitmCertProvider, BoxError> {
    let alg = &PKCS_ECDSA_P256_SHA256;
    let ca_key_pair = KeyPair::generate_for(alg).context("generate CA key pair")?;

    let mut ca_params = CertificateParams::new(Vec::new()).context("create CA params")?;
    ca_params
        .distinguished_name
        .push(DnType::OrganizationName, CA_ORG_NAME);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, CA_COMMON_NAME);
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::CrlSign,
    ];

    let ca_cert = ca_params
        .self_signed(&ca_key_pair)
        .context("self-signed CA cert")?;

    let ca_key_pem = ca_key_pair.serialize_pem();
    let ca_cert_pem_str = ca_cert.pem();

    std::fs::write(ca_cert_path(dir), &ca_cert_pem_str).context("write CA cert PEM")?;
    std::fs::write(ca_key_path(dir), &ca_key_pem).context("write CA key PEM")?;

    log::info!(
        "New CA certificate generated and saved to {}",
        ca_cert_path(dir).display()
    );

    let ca_cert_der: CertificateDer = ca_cert.into();

    Ok(MitmCertProvider {
        ca_key_pem,
        ca_cert_der,
    })
}

fn create_ca_issuer(ca_key_pair: KeyPair) -> Issuer<'static, KeyPair> {
    let mut ca_params = CertificateParams::new(Vec::new()).expect("create CA params");
    ca_params
        .distinguished_name
        .push(DnType::OrganizationName, CA_ORG_NAME);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, CA_COMMON_NAME);
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::CrlSign,
    ];
    Issuer::new(ca_params, ca_key_pair)
}

fn generate_server_cert_for_hostname(
    ca_issuer: &Issuer<KeyPair>,
    hostname: &str,
) -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), BoxError> {
    let alg = &PKCS_ECDSA_P256_SHA256;
    let server_key_pair = KeyPair::generate_for(alg).context("generate server key pair")?;

    let mut server_params =
        CertificateParams::new(vec![hostname.to_string()]).context("create server EE params")?;
    server_params
        .distinguished_name
        .push(DnType::CommonName, hostname);
    server_params.is_ca = IsCa::NoCa;
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];

    let server_cert = server_params
        .signed_by(&server_key_pair, ca_issuer)
        .context("sign server cert")?;

    let server_cert_der: CertificateDer = server_cert.into();
    let server_key_der: PrivateKeyDer =
        PrivatePkcs8KeyDer::from(server_key_pair.serialize_der()).into();

    Ok((server_cert_der, server_key_der))
}

fn pem_to_cert_der(pem: &str) -> CertificateDer<'static> {
    let b64: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    let der = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
        .expect("valid base64 in PEM");
    CertificateDer::from(der)
}

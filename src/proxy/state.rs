use rama::extensions::Extension;
use rama::rt::Executor;
use rama::tls::rustls::server::TlsAcceptorData;

#[derive(Debug, Clone, Extension)]
pub(crate) struct State {
    pub(crate) mitm_tls_service_data: TlsAcceptorData,
    pub(crate) exec: Executor,
}
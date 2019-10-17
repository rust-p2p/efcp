use failure::Fail;

#[derive(Debug, Fail)]
pub enum HandshakeError {
    #[fail(display = "{}", _0)]
    Io(std::io::Error),
    #[fail(display = "{}", _0)]
    Disco(disco::ReadError),
    #[fail(display = "protocol error")]
    ProtocolError,
    #[fail(display = "protocol negotiation failed")]
    Negotiation,
    #[fail(display = "no external addr received")]
    ExternalAddr,
}

impl From<std::io::Error> for HandshakeError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<disco::ReadError> for HandshakeError {
    fn from(err: disco::ReadError) -> Self {
        Self::Disco(err)
    }
}

//! # Error and flow control protocol
//! Initially a derivation of the delta-t transport protocol for use in the
//! RINA network architecture. It tries to stay true to the concepts but
//! adapted to the UDP/IP infrastructure. In addition encryption based on
//! the noise and disco specifications are mandated, encryption being an
//! optional feature in the original EFCP incarnation.
//!
//! ## Security considerations
//! Padding: All messages must be padded to equal length to prevent information
//!   leakage.
//! Termination: Session termination must be made explicit to prevent truncation
//!   attacks.
//! Nonces: Every message has a clear text nonce. Nonces are not allowed to wrap
//!   since that would open the possibility of messages being replayed. For this
//!   reason the nonce must be  minimum 64 bits in length. The assumption is
//!   made that with todays technology a nonce of 64-bits will never wrap.
//!
//! ## Handshake
//! The default handshake is based on the `XK1sig` pattern from the noise
//! signature extension spec. The client identity is useful for performing
//! access control operations.
//!
//! XK1sig
//!   <- s
//!   ...
//!   -> e
//!   <- e ee sig
//!   -> s sig
//!
//!
//! ## Observed address and NAT traversal
//! In P2P networks knowledge of the external address port tuple is neccessary
//! to advertise availability on a DHT. For this reason during a handshake
//! message the first response from a responder must include the observed
//! address and port.
//!
//! ## Generic protocol negotiation
//! Based on the libp2p connection spec, a protocol identifier is sent by the
//! initiator to request a protocol. The responder can accept by echoing the
//! protocol identifier or deny with a N/A message. On failure the negotiation
//! can be restarted. This process is (by default) started in parallel with the
//! handshake to reduce latency.
//!
//! ### DTCP parameter negotiation
//! The congestion and flow control elements of the protocol have different
//! parameters to make it tunable to a specific application. These are
//! negotiated using the generic protocol negotiation mechanism.
//!
//! ### Application protocol negotiation
//! The application protocol has a unique protocol identifier, which is used
//! to negotiate the application protocol using the generic protocol negotiation
//! mechanism.
pub mod negotiation;

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

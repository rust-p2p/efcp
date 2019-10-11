//! Data Transfer Protocol
use crate::constants::SequenceNumber;
use crate::dtcp::Dtcp;
use crate::packet::Packet;
use failure::Fail;
use std::collections::VecDeque;

#[derive(Debug, Fail)]
pub enum SendError {
    /// Once a sequence number rollover occurs we can no longer guarantee the
    /// reliability of the transport. A malicious user could replay a previous
    /// message even if encryption is enabled on an upper layer.
    #[fail(display = "sequence number rollover occured")]
    SequenceNumberRollover,
    /// Max closed window queue length exceeded is used to notify an upper
    /// layer that it should throttle it's sending rate.
    #[fail(display = "max closed window queue length exceeded")]
    MaxClosedWindowQueue,
}

/// State of the flow
pub enum FlowState {
    /// Flow is initializing.
    Null,
    /// Flow is ready for sending and receiving PDUs.
    Active,
    /// Transitioning to a different flow to prevent sequence number rollover.
    FlowTransition,
}

/// Dtp state machine.
pub struct Dtp {
    /// Maximum SDU size for this connection.
    max_flow_sdu_size: u64,
    /// Maximum PDU size for this connection.
    max_flow_pdu_size: u64,
    /// State of the flow.
    state: FlowState,
    /// DTCP handler.
    dtcp: Option<Dtcp>,
    /// Indicates if the flow control window is closed.
    closed_window: bool,
    /// Indicates that with rate based flow control all the PDUs that can be
    /// sent during this time period have been sent.
    rate_fulfilled: bool,
    /// Maximum number of PDUs queued to send because the flow control window
    /// is closed.
    max_closed_window_queue_len: usize,
    /// Indicates if the SDUs can be delivered incrementally.
    partial_delivery: bool,
    /// Indicates if SDUs with missing fragments can be delivered.
    incomplete_delivery: bool,
    /// Queue of PDUs ready to be sent once the window opens.
    closed_window_queue: VecDeque<Packet>,
    /// Largest sequence number received.
    max_sequence_number_received: SequenceNumber,
    /// Next sequence number.
    sequence_number: SequenceNumber,
    /// Queue of PDUs requiring reassembly.
    reassembly_queue: VecDeque<(Packet, SequenceNumber)>,
}

impl Dtp {
    pub fn send_packet(&mut self, payload: &[u8]) -> Result<(), SendError> {
        // stop sender inactivity timer

        // create packet
        let mut packet = Packet::dtp(payload);
        // sequence number
        packet.set_sequence_number(self.sequence_number);
        self.sequence_number += 1;

        if let Some(dtcp) = self.dtcp.as_mut() {
            // set data run flag
            packet.set_drf(dtcp.take_drf());
            if dtcp.window_open(packet.sequence_number()) {
                dtcp.register_packet(&packet)
                // TODO send packet
            } else {
                self.closed_window_queue.push_back(packet);
            }
        } else {
            // TODO send packet
        }

        // start sender inactivity timer

        // notify the caller about error conditions.
        if self.closed_window_queue.len() > self.max_closed_window_queue_len {
            return Err(SendError::MaxClosedWindowQueue);
        }

        Ok(())
    }
}

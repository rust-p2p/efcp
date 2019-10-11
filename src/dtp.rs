//! Data Transfer Protocol
use crate::constants::{Instant, SequenceNumber};
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
    /// The sequence number at which a new flow needs to be established
    /// to prevent sequence number rollover.
    sequence_number_roll_over_threshold: SequenceNumber,
    /// State of the flow.
    state: FlowState,
    /// DTCP enabled.
    dtcp: Option<Dtcp>,
    /// Window based flow control enabled.
    window_flow_control: bool,
    /// Rate based flow control enabled.
    rate_flow_control: bool,
    /// Retransmission enabled.
    retransmission_present: bool,
    /// Indicates if the flow control window is closed.
    closed_window: bool,
    /// Indicates that with rate based flow control all the PDUs that can be
    /// sent during this time period have been sent.
    rate_fulfilled: bool,
    /// Number of PDUs queued to send because the flow control window is
    /// closed.
    closed_window_length: i64,
    /// Maximum number of PDUs queued to send because the flow control window
    /// is closed.
    max_closed_window_queue_len: i64,
    /// Indicates if the SDUs can be delivered incrementally.
    partial_delivery: bool,
    /// Indicates if SDUs with missing fragments can be delivered.
    incomplete_delivery: bool,
    /// Queue of PDUs ready to be sent once the window opens.
    closed_window_queue: VecDeque<Packet>,
    /// Largest sequence number that we acknowledged.
    received_left_window_edge: SequenceNumber,
    /// Largest sequence number received.
    max_sequence_number_received: SequenceNumber,
    /// Largest sequence number that has been acknowledged.
    sender_left_window_edge: SequenceNumber,
    /// Next sequence number.
    sequence_number: SequenceNumber,
    /// Queue of PDUs requiring reassembly.
    pdu_reassembly_queue: VecDeque<(Packet, SequenceNumber)>,
}

impl Dtp {
    pub fn send_packet(&mut self, payload: &[u8]) -> Result<(), SendError> {
        // stop sender inactivity timer

        // create packet
        let mut packet = Packet::dtp(payload);
        // sequence number
        packet.set_sequence_number(self.sequence_number);
        self.sequence_number += 1;

        if let Some(mut dtcp) = self.dtcp {
            // set data run flag
            packet.set_drf(dtcp.take_drf());

            if self.window_flow_control {
                self.closed_window = packet.sequence_number() <= self.right_window_edge;
            }
            if self.rate_flow_control {
                self.rate_fulfilled = self.pdus_sent < self.sending_rate;
            }
            if self.closed_window || self.rate_fulfilled {
                self.closed_window_queue.push_back(packet);
            } else {
                // TODO update sending rate measurement
                // self.pdus_sent += 1;
                if self.retransmission_present {
                    dtcp.register_retransmission(&packet);
                }
                // TODO send packet
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

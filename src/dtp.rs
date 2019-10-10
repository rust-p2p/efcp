//! Data Transfer Protocol
use crate::packet::{Packet, SequenceNumber};
use std::collections::VecDeque;

struct UserData;
struct Timer;

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
#[allow(dead_code)]
pub struct DTP {
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
    dtcp_present: bool,
    /// Window based flow control enabled.
    window_based: bool,
    /// Rate based flow control enabled.
    rate_based: bool,
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
    /// Queue of sent packets that have not yet been acknowledged.
    retransmission_queue: VecDeque<(Packet, Timer)>,
    /// Queue of PDUs ready to be sent once the window opens.
    closed_window_queue: VecDeque<Packet>,
    /// Largest sequence number that we acknowledged.
    received_left_window_edge: SequenceNumber,
    /// Largest sequence number received.
    max_sequence_number_received: SequenceNumber,
    /// Largest sequence number that has been acknowledged.
    sender_left_window_edge: SequenceNumber,
    /// Next sequence number.
    next_sequence_number: SequenceNumber,
    /// Queue of PDUs requiring reassembly.
    pdu_reassembly_queue: VecDeque<(Packet, SequenceNumber)>,
    /// Queue of PDU payloads waiting for transmission.
    user_data_queue: VecDeque<UserData>,
}



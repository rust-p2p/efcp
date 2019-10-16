pub type Protocol = &'static str;
pub type Protocols = &'static [Protocol];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    Propose(&'static str),
    Accept(&'static str),
    Fail,
}

#[derive(Debug)]
pub struct ProtocolError;

pub struct Negotiation {
    protocols: Protocols,
    tried: usize,
    started: bool,
    proposed: Option<&'static str>,
    accepted: Option<&'static str>,
    finished: bool,
}

impl Negotiation {
    pub fn new(protocols: Protocols) -> Self {
        Self {
            protocols,
            started: false,
            tried: 0,
            proposed: None,
            accepted: None,
            finished: false,
        }
    }

    fn supported(&self, protocol: Protocol) -> bool {
        self.protocols.iter().find(|p| **p == protocol).is_some()
    }

    fn propose(&mut self) -> Message {
        let i = self.tried;
        self.tried += 1;
        if let Some(protocol) = self.protocols.get(i) {
            self.proposed = Some(protocol);
            Message::Propose(protocol)
        } else {
            self.finished = true;
            Message::Fail
        }
    }

    pub fn initiate(&mut self) -> Message {
        assert!(!self.started);
        self.started = true;
        self.propose()
    }

    pub fn message(&mut self, msg: Message) -> Result<Option<Message>, ProtocolError> {
        self.started = true;
        match msg {
            Message::Propose(protocol) => {
                if self.accepted.is_some() {
                    return Err(ProtocolError);
                }
                
                if self.supported(protocol) {
                    self.accepted = Some(protocol);
                    self.finished = true;
                    Ok(Some(Message::Accept(protocol)))
                } else {
                    Ok(Some(self.propose()))
                }
            }
            Message::Accept(protocol) => {
                if self.supported(protocol) {
                    self.accepted = Some(protocol);
                    self.finished = true;
                    Ok(None)
                } else {
                    Err(ProtocolError)
                }
            }
            Message::Fail => {
                self.finished = true;
                Ok(None)
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn into_protocol(self) -> Option<Protocol> {
        assert!(self.finished);
        self.accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_proto_common() {
        let mut n1 = Negotiation::new(&["/ping/1.0"]);
        let mut n2 = Negotiation::new(&["/ping/1.0"]);
        let m1 = n1.initiate();
        assert_eq!(m1, Message::Propose("/ping/1.0"));
        let m2 = n2.message(m1).unwrap().unwrap();
        assert_eq!(m2, Message::Accept("/ping/1.0"));
        let m3 = n1.message(m2).unwrap();
        assert_eq!(m3, None);
        
        assert!(n1.is_finished());
        assert!(n2.is_finished());

        let p1 = n1.into_protocol();
        assert_eq!(p1, Some("/ping/1.0"));
        let p2 = n2.into_protocol();
        assert_eq!(p1, p2);
    }

    #[test]
    fn no_proto_common() {
        let mut n1 = Negotiation::new(&["/ping/1.0"]);
        let mut n2 = Negotiation::new(&["/ping/2.0"]);
        
        let m1 = n1.initiate();
        assert_eq!(m1, Message::Propose("/ping/1.0"));
        
        let m2 = n2.message(m1).unwrap().unwrap();
        assert_eq!(m2, Message::Propose("/ping/2.0"));
        
        let m3 = n1.message(m2).unwrap().unwrap();
        assert_eq!(m3, Message::Fail);

        let m4 = n2.message(m3).unwrap();
        assert_eq!(m4, None);
        
        assert!(n1.is_finished());
        assert!(n2.is_finished());

        let p1 = n1.into_protocol();
        assert_eq!(p1, None);
        let p2 = n2.into_protocol();
        assert_eq!(p1, p2);
    }

    #[test]
    fn one_proto_common() {
        let mut n1 = Negotiation::new(&["/ping/2.0", "/ping/1.0"]);
        let mut n2 = Negotiation::new(&["/ping/1.0"]);
        
        let m1 = n1.initiate();
        assert_eq!(m1, Message::Propose("/ping/2.0"));
        
        let m2 = n2.message(m1).unwrap().unwrap();
        assert_eq!(m2, Message::Propose("/ping/1.0"));
        
        let m3 = n1.message(m2).unwrap().unwrap();
        assert_eq!(m3, Message::Accept("/ping/1.0"));

        let m4 = n2.message(m3).unwrap();
        assert_eq!(m4, None);
        
        assert!(n1.is_finished());
        assert!(n2.is_finished());

        let p1 = n1.into_protocol();
        assert_eq!(p1, Some("/ping/1.0"));
        let p2 = n2.into_protocol();
        assert_eq!(p1, p2);
    }

    /*#[test]
    fn both_initiate() {
        let mut n1 = Negotiation::new(&["/ping/2.0", "/ping/1.0"]);
        let mut n2 = Negotiation::new(&["/ping/1.0", "/ping/2.0"]);
        
        let m1 = n1.initiate();
        assert_eq!(m1, Message::Propose("/ping/2.0"));
        
        let m2 = n2.initiate();
        assert_eq!(m2, Message::Propose("/ping/1.0"));
        
        let m3 = n1.message(m2).unwrap().unwrap();
        assert_eq!(m3, Message::Accept("/ping/1.0"));
        
        let m4 = n2.message(m1).unwrap().unwrap();
        assert_eq!(m4, Message::Accept("/ping/2.0"));

        let m5 = n1.message(m4).unwrap().unwrap();
        let m6 = n2.message(m3).unwrap().unwrap()
        
        assert!(n1.is_finished());
        assert!(n2.is_finished());

        let p1 = n1.into_protocol();
        assert_eq!(p1, Some("/ping/1.0"));
        let p2 = n2.into_protocol();
        assert_eq!(p1, p2);
    }*/
}

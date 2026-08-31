//! Modbus RTU **slave** simulator: the device end of the RS-485 seam.
//!
//! flare-edge's `warden-modbus` is the *master/scanner*: it probes RS-485 for
//! VFDs and PDUs, identifies them, and reads their register maps. To harden that
//! master to MC/DC we need something for it to talk to that behaves like a real
//! slave: correct CRC framing, the data-plane function codes, exception replies,
//! and the annoying real-world faults (a cheap sensor that ignores a function, a
//! device that NAKs an unsupported code). This is that slave, in host memory:
//! feed it a request frame, get the response frame (or `None` when a real slave
//! would stay silent). No serial port, no hardware, fully deterministic.
//!
//! Scope is the data plane: read/write of holding & input registers, coils, and
//! discrete inputs (FC 0x01-0x06, 0x0F, 0x10) plus Report Slave ID (0x11), which
//! is what a VFD/PDU register poll actually exercises. Identification via MEI
//! (0x2B/0x0E) is a documented follow-up.

/// Modbus exception codes (returned as `fc | 0x80`, then the code).
pub mod exc {
    pub const ILLEGAL_FUNCTION: u8 = 0x01;
    pub const ILLEGAL_DATA_ADDRESS: u8 = 0x02;
    pub const ILLEGAL_DATA_VALUE: u8 = 0x03;
}

/// Modbus RTU CRC16 (poly 0xA001, low byte first on the wire). Identical to the
/// master's `crc16`: the two must agree or nothing frames.
pub fn crc16(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in bytes {
        crc ^= b as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

/// Append the RTU CRC (low byte then high) to a frame body in place.
pub fn append_crc(frame: &mut Vec<u8>) {
    let c = crc16(frame);
    frame.push((c & 0xFF) as u8);
    frame.push((c >> 8) as u8);
}

/// True if `frame` (including its trailing 2 CRC bytes) has a valid CRC.
pub fn crc_ok(frame: &[u8]) -> bool {
    if frame.len() < 3 {
        return false;
    }
    let (body, crc) = frame.split_at(frame.len() - 2);
    crc == [(crc16(body) & 0xFF) as u8, (crc16(body) >> 8) as u8]
}

/// A Modbus RTU slave device model.
pub struct ModbusSlave {
    pub address: u8,
    holding: Vec<u16>,
    input: Vec<u16>,
    coils: Vec<bool>,
    discrete: Vec<bool>,
    slave_id: Vec<u8>,
    /// Silently drop this many upcoming requests (models a device that ignores a
    /// function, or a flaky bus): the master must time out and move on.
    drop_next: usize,
    /// Force every function to answer with this exception (models a device that
    /// NAKs everything but a narrow set) until cleared.
    force_exception: Option<u8>,
}

impl ModbusSlave {
    /// A slave at `address` with `regs` holding & input registers and `bits`
    /// coils & discrete inputs, all zero.
    pub fn new(address: u8, regs: usize, bits: usize) -> Self {
        Self {
            address,
            holding: vec![0; regs],
            input: vec![0; regs],
            coils: vec![false; bits],
            discrete: vec![false; bits],
            slave_id: vec![0xFF, 0xFF], // run-indicator on; arbitrary id byte
            drop_next: 0,
            force_exception: None,
        }
    }

    pub fn set_holding(&mut self, addr: usize, v: u16) {
        self.holding[addr] = v;
    }
    pub fn set_input(&mut self, addr: usize, v: u16) {
        self.input[addr] = v;
    }
    pub fn set_coil(&mut self, addr: usize, v: bool) {
        self.coils[addr] = v;
    }
    pub fn set_discrete(&mut self, addr: usize, v: bool) {
        self.discrete[addr] = v;
    }
    pub fn holding(&self, addr: usize) -> u16 {
        self.holding[addr]
    }
    pub fn coil(&self, addr: usize) -> bool {
        self.coils[addr]
    }
    /// Set the Report-Slave-ID payload (everything after the byte-count).
    pub fn set_slave_id(&mut self, id: Vec<u8>) {
        self.slave_id = id;
    }

    /// Ignore the next `n` requests entirely (no response).
    pub fn drop_next(&mut self, n: usize) {
        self.drop_next = n;
    }
    /// Answer every request with `code` until [`clear_faults`](Self::clear_faults).
    pub fn force_exception(&mut self, code: u8) {
        self.force_exception = Some(code);
    }
    pub fn clear_faults(&mut self) {
        self.drop_next = 0;
        self.force_exception = None;
    }

    /// Handle one RTU request frame; return the RTU response frame, or `None`
    /// when a real slave would stay silent (bad CRC, wrong address, broadcast,
    /// or an injected drop).
    pub fn handle_frame(&mut self, req: &[u8]) -> Option<Vec<u8>> {
        if self.drop_next > 0 {
            self.drop_next -= 1;
            return None;
        }
        if req.len() < 4 || !crc_ok(req) {
            return None; // RTU slaves silently discard malformed / bad-CRC frames
        }
        let addr = req[0];
        if addr != self.address {
            return None; // not for us (address 0 = broadcast: no reply either)
        }
        let fc = req[1];
        let pdu = &req[2..req.len() - 2]; // between address and CRC

        let body = if let Some(code) = self.force_exception {
            Err(code)
        } else {
            self.dispatch(fc, pdu)
        };

        let mut frame = vec![self.address];
        match body {
            Ok(mut data) => {
                frame.push(fc);
                frame.append(&mut data);
            }
            Err(code) => {
                frame.push(fc | 0x80);
                frame.push(code);
            }
        }
        append_crc(&mut frame);
        Some(frame)
    }

    /// Build the response PDU (everything between fc and CRC) or an exception.
    fn dispatch(&mut self, fc: u8, pdu: &[u8]) -> Result<Vec<u8>, u8> {
        match fc {
            0x01 => self.read_bits(pdu, false),
            0x02 => self.read_bits(pdu, true),
            0x03 => self.read_regs(pdu, false),
            0x04 => self.read_regs(pdu, true),
            0x05 => self.write_single_coil(pdu),
            0x06 => self.write_single_reg(pdu),
            0x0F => self.write_multi_coils(pdu),
            0x10 => self.write_multi_regs(pdu),
            0x11 => Ok(self.report_slave_id()),
            _ => Err(exc::ILLEGAL_FUNCTION),
        }
    }

    fn read_regs(&self, pdu: &[u8], input: bool) -> Result<Vec<u8>, u8> {
        if pdu.len() != 4 {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        let start = u16::from_be_bytes([pdu[0], pdu[1]]) as usize;
        let count = u16::from_be_bytes([pdu[2], pdu[3]]) as usize;
        if count == 0 || count > 125 {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        let bank = if input { &self.input } else { &self.holding };
        if start + count > bank.len() {
            return Err(exc::ILLEGAL_DATA_ADDRESS);
        }
        let mut out = vec![(count * 2) as u8];
        for r in &bank[start..start + count] {
            out.extend_from_slice(&r.to_be_bytes());
        }
        Ok(out)
    }

    fn read_bits(&self, pdu: &[u8], discrete: bool) -> Result<Vec<u8>, u8> {
        if pdu.len() != 4 {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        let start = u16::from_be_bytes([pdu[0], pdu[1]]) as usize;
        let count = u16::from_be_bytes([pdu[2], pdu[3]]) as usize;
        if count == 0 || count > 2000 {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        let bank = if discrete {
            &self.discrete
        } else {
            &self.coils
        };
        if start + count > bank.len() {
            return Err(exc::ILLEGAL_DATA_ADDRESS);
        }
        let nbytes = count.div_ceil(8);
        let mut out = vec![nbytes as u8];
        out.extend(std::iter::repeat_n(0u8, nbytes));
        for (i, &bit) in bank[start..start + count].iter().enumerate() {
            if bit {
                out[1 + i / 8] |= 1 << (i % 8);
            }
        }
        Ok(out)
    }

    fn write_single_reg(&mut self, pdu: &[u8]) -> Result<Vec<u8>, u8> {
        if pdu.len() != 4 {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        let addr = u16::from_be_bytes([pdu[0], pdu[1]]) as usize;
        let val = u16::from_be_bytes([pdu[2], pdu[3]]);
        if addr >= self.holding.len() {
            return Err(exc::ILLEGAL_DATA_ADDRESS);
        }
        self.holding[addr] = val;
        Ok(pdu.to_vec()) // echo request PDU
    }

    fn write_single_coil(&mut self, pdu: &[u8]) -> Result<Vec<u8>, u8> {
        if pdu.len() != 4 {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        let addr = u16::from_be_bytes([pdu[0], pdu[1]]) as usize;
        let val = u16::from_be_bytes([pdu[2], pdu[3]]);
        if val != 0x0000 && val != 0xFF00 {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        if addr >= self.coils.len() {
            return Err(exc::ILLEGAL_DATA_ADDRESS);
        }
        self.coils[addr] = val == 0xFF00;
        Ok(pdu.to_vec())
    }

    fn write_multi_regs(&mut self, pdu: &[u8]) -> Result<Vec<u8>, u8> {
        if pdu.len() < 5 {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        let start = u16::from_be_bytes([pdu[0], pdu[1]]) as usize;
        let count = u16::from_be_bytes([pdu[2], pdu[3]]) as usize;
        let bytecount = pdu[4] as usize;
        if count == 0 || count > 123 || bytecount != count * 2 || pdu.len() != 5 + bytecount {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        if start + count > self.holding.len() {
            return Err(exc::ILLEGAL_DATA_ADDRESS);
        }
        for i in 0..count {
            self.holding[start + i] = u16::from_be_bytes([pdu[5 + i * 2], pdu[6 + i * 2]]);
        }
        Ok(pdu[0..4].to_vec()) // echo address + count
    }

    fn write_multi_coils(&mut self, pdu: &[u8]) -> Result<Vec<u8>, u8> {
        if pdu.len() < 5 {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        let start = u16::from_be_bytes([pdu[0], pdu[1]]) as usize;
        let count = u16::from_be_bytes([pdu[2], pdu[3]]) as usize;
        let bytecount = pdu[4] as usize;
        if count == 0
            || count > 1968
            || bytecount != count.div_ceil(8)
            || pdu.len() != 5 + bytecount
        {
            return Err(exc::ILLEGAL_DATA_VALUE);
        }
        if start + count > self.coils.len() {
            return Err(exc::ILLEGAL_DATA_ADDRESS);
        }
        for i in 0..count {
            self.coils[start + i] = pdu[5 + i / 8] & (1 << (i % 8)) != 0;
        }
        Ok(pdu[0..4].to_vec())
    }

    fn report_slave_id(&self) -> Vec<u8> {
        let mut out = vec![self.slave_id.len() as u8];
        out.extend_from_slice(&self.slave_id);
        out
    }
}

/// Build an RTU request frame (with CRC): convenience for tests and for driving
/// the master's parser. `pdu` is everything between the address and the CRC
/// (i.e. `fc` followed by its data).
pub fn request(address: u8, pdu: &[u8]) -> Vec<u8> {
    let mut f = vec![address];
    f.extend_from_slice(pdu);
    append_crc(&mut f);
    f
}

/// Build a read-holding-registers (FC 0x03) request frame.
pub fn read_holding(address: u8, start: u16, count: u16) -> Vec<u8> {
    let mut pdu = vec![0x03];
    pdu.extend_from_slice(&start.to_be_bytes());
    pdu.extend_from_slice(&count.to_be_bytes());
    request(address, &pdu)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known Modbus RTU CRC vector: `01 03 00 00 00 01` -> low `84`, high `0A`.
    #[test]
    fn crc_known_vector() {
        assert_eq!(crc16(&[0x01, 0x03, 0x00, 0x00, 0x00, 0x01]), 0x0A84);
        let f = read_holding(1, 0, 1);
        assert_eq!(&f[f.len() - 2..], &[0x84, 0x0A]);
        assert!(crc_ok(&f));
    }

    #[test]
    fn read_holding_returns_values() {
        let mut s = ModbusSlave::new(0x11, 16, 0);
        s.set_holding(2, 0xABCD);
        s.set_holding(3, 0x1234);
        let resp = s.handle_frame(&read_holding(0x11, 2, 2)).unwrap();
        // addr fc bytecount d d d d crc crc
        assert_eq!(resp[0], 0x11);
        assert_eq!(resp[1], 0x03);
        assert_eq!(resp[2], 4); // 2 regs * 2 bytes
        assert_eq!(&resp[3..7], &[0xAB, 0xCD, 0x12, 0x34]);
        assert!(crc_ok(&resp));
    }

    #[test]
    fn write_single_register_updates_and_echoes() {
        let mut s = ModbusSlave::new(1, 8, 0);
        let req = request(1, &[0x06, 0x00, 0x05, 0x07, 0xD0]); // write 2000 -> reg 5
        let resp = s.handle_frame(&req).unwrap();
        assert_eq!(s.holding(5), 2000);
        assert_eq!(&resp[1..6], &[0x06, 0x00, 0x05, 0x07, 0xD0]); // echo
    }

    #[test]
    fn write_multiple_registers() {
        let mut s = ModbusSlave::new(1, 8, 0);
        // FC10 start=0 count=2 bytecount=4 vals=0x1111,0x2222
        let req = request(1, &[0x10, 0, 0, 0, 2, 4, 0x11, 0x11, 0x22, 0x22]);
        let resp = s.handle_frame(&req).unwrap();
        assert_eq!(s.holding(0), 0x1111);
        assert_eq!(s.holding(1), 0x2222);
        assert_eq!(&resp[1..6], &[0x10, 0, 0, 0, 2]); // echo addr+count
    }

    #[test]
    fn coils_write_then_read() {
        let mut s = ModbusSlave::new(1, 0, 16);
        // FC05 write single coil 3 = ON
        s.handle_frame(&request(1, &[0x05, 0, 3, 0xFF, 0x00]))
            .unwrap();
        assert!(s.coil(3));
        // FC01 read coils 0..8 -> bit 3 set => byte 0x08
        let resp = s.handle_frame(&request(1, &[0x01, 0, 0, 0, 8])).unwrap();
        assert_eq!(resp[2], 1); // 1 byte
        assert_eq!(resp[3], 0x08);
    }

    #[test]
    fn illegal_function_returns_exception() {
        let mut s = ModbusSlave::new(1, 8, 0);
        let resp = s.handle_frame(&request(1, &[0x63, 0, 0])).unwrap();
        assert_eq!(resp[1], 0x63 | 0x80);
        assert_eq!(resp[2], exc::ILLEGAL_FUNCTION);
        assert!(crc_ok(&resp));
    }

    #[test]
    fn out_of_range_read_is_illegal_data_address() {
        let mut s = ModbusSlave::new(1, 4, 0);
        let resp = s.handle_frame(&read_holding(1, 2, 10)).unwrap();
        assert_eq!(resp[1], 0x83);
        assert_eq!(resp[2], exc::ILLEGAL_DATA_ADDRESS);
    }

    #[test]
    fn wrong_address_and_bad_crc_are_silent() {
        let mut s = ModbusSlave::new(0x11, 8, 0);
        assert!(s.handle_frame(&read_holding(0x12, 0, 1)).is_none()); // other slave
        let mut bad = read_holding(0x11, 0, 1);
        *bad.last_mut().unwrap() ^= 0xFF; // corrupt CRC
        assert!(s.handle_frame(&bad).is_none());
    }

    /// A device that ignores the next request (the "cheap sensor" the master
    /// comment warns about): the master must fall through to the next function.
    #[test]
    fn drop_next_models_a_silent_device() {
        let mut s = ModbusSlave::new(1, 8, 0);
        s.drop_next(1);
        assert!(s.handle_frame(&read_holding(1, 0, 1)).is_none()); // ignored
        assert!(s.handle_frame(&read_holding(1, 0, 1)).is_some()); // recovers
    }

    /// A device that NAKs everything (forced exception) until cleared.
    #[test]
    fn force_exception_naks_until_cleared() {
        let mut s = ModbusSlave::new(1, 8, 0);
        s.force_exception(exc::ILLEGAL_FUNCTION);
        let resp = s.handle_frame(&read_holding(1, 0, 1)).unwrap();
        assert_eq!(resp[1], 0x83);
        s.clear_faults();
        let resp = s.handle_frame(&read_holding(1, 0, 1)).unwrap();
        assert_eq!(resp[1], 0x03); // normal again
    }

    #[test]
    fn report_slave_id() {
        let mut s = ModbusSlave::new(7, 4, 0);
        s.set_slave_id(vec![0x42, 0xFF]);
        let resp = s.handle_frame(&request(7, &[0x11])).unwrap();
        assert_eq!(resp[1], 0x11);
        assert_eq!(resp[2], 2); // byte count
        assert_eq!(&resp[3..5], &[0x42, 0xFF]);
    }
}

use crate::decoder::DecodingError;

#[must_use]
#[repr(transparent)]
pub(crate) struct BitResult<T> {
    value_if_not_past_eof: T,
}

#[must_use]
pub(crate) struct BitResultAccumulator;

impl BitResult<()> {
    pub(crate) const OK: BitResultAccumulator = BitResultAccumulator;
}

impl<T> BitResult<T> {
    const fn ok(value: T) -> Self {
        Self {
            value_if_not_past_eof: value,
        }
    }

    #[inline(always)]
    pub(crate) fn or_accumulate(self, acc: &mut BitResultAccumulator) -> T {
        let _ = acc;
        self.value_if_not_past_eof
    }
}

impl<T: Default> BitResult<T> {
    fn err() -> Self {
        Self {
            value_if_not_past_eof: T::default(),
        }
    }
}

#[cfg_attr(test, derive(Debug))]
pub(crate) struct BoolReader {
    chunks: Box<[[u8; 4]]>,
    chunk_index: usize,
    value: u64,
    range: u32,
    bit_count: i32,
    final_bytes: [u8; 4],
    final_bytes_remaining: i32,
}

impl BoolReader {
    pub(crate) fn new() -> BoolReader {
        BoolReader {
            chunks: Box::new([]),
            chunk_index: 0,
            value: 0,
            range: 0,
            bit_count: 0,
            final_bytes: [0; 4],
            final_bytes_remaining: 0,
        }
    }

    pub(crate) fn init(&mut self, mut buf: Vec<[u8; 4]>, len: usize) -> Result<(), DecodingError> {
        // Pop the last chunk (which may be partial), then get length.
        let Some(last_chunk) = buf.pop() else {
            return Err(DecodingError::NotEnoughInitData);
        };
        let len_rounded_down = 4 * buf.len();
        let num_bytes_popped = len - len_rounded_down;
        debug_assert!(num_bytes_popped <= 4);
        for i in num_bytes_popped..4 {
            debug_assert_eq!(last_chunk[i], 0, "unexpected {last_chunk:?}");
        }

        let chunks = buf.into_boxed_slice();
        *self = Self {
            chunks,
            chunk_index: 0,
            value: 0,
            range: 255,
            bit_count: -8,
            final_bytes: last_chunk,
            final_bytes_remaining: num_bytes_popped as i32,
        };
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn check<T>(
        &self,
        acc: BitResultAccumulator,
        value_if_not_past_eof: T,
    ) -> Result<T, DecodingError> {
        let _ = acc;
        if self.is_past_eof() {
            Err(DecodingError::BitStreamError)
        } else {
            Ok(value_if_not_past_eof)
        }
    }

    #[inline(always)]
    pub(crate) fn check_directly<T>(&self, result: BitResult<T>) -> Result<T, DecodingError> {
        let mut acc = BitResult::OK;
        let value = result.or_accumulate(&mut acc);
        self.check(acc, value)
    }

    fn accumulated<T>(&self, acc: BitResultAccumulator, value_if_not_past_eof: T) -> BitResult<T> {
        let _ = acc;
        BitResult::ok(value_if_not_past_eof)
    }

    const FINAL_BYTES_REMAINING_EOF: i32 = -0xE0F;

    #[cold]
    fn load_final_bytes(&mut self) {
        if self.final_bytes_remaining > 0 {
            self.final_bytes_remaining -= 1;
            let byte = self.final_bytes[0];
            self.final_bytes.rotate_left(1);
            self.value <<= 8;
            self.value |= u64::from(byte);
            self.bit_count += 8;
        } else if self.final_bytes_remaining == 0 {
            // libwebp seems to (sometimes?) allow bitstreams that read one byte past the end.
            // This replicates that logic.
            self.final_bytes_remaining -= 1;
            self.value <<= 8;
            self.bit_count += 8;
        } else {
            self.final_bytes_remaining = Self::FINAL_BYTES_REMAINING_EOF;
        }
    }

    fn is_past_eof(&self) -> bool {
        self.final_bytes_remaining == Self::FINAL_BYTES_REMAINING_EOF
    }

    #[cold]
    #[inline(never)]
    fn cold_read_bit_from_final_bytes(&mut self, probability: u32) -> BitResult<bool> {
        if self.bit_count < 0 {
            self.load_final_bytes();
            if self.is_past_eof() {
                return BitResult::err();
            }
        }
        debug_assert!(self.bit_count >= 0);

        let split = 1 + (((self.range - 1) * probability) >> 8);
        let bigsplit = u64::from(split) << self.bit_count;

        let retval = if let Some(new_value) = self.value.checked_sub(bigsplit) {
            self.range -= split;
            self.value = new_value;
            true
        } else {
            self.range = split;
            false
        };
        debug_assert!(self.range > 0);

        // Compute shift required to satisfy `self.range >= 128`.
        // Apply that shift to `self.range` and `self.bitcount`.
        //
        // Subtract 24 because we only care about leading zeros in the
        // lowest byte of `self.range` which is a `u32`.
        let shift = self.range.leading_zeros().saturating_sub(24);
        self.range <<= shift;
        self.bit_count -= shift as i32;
        debug_assert!(self.range >= 128);

        BitResult::ok(retval)
    }

    fn read_bit(&mut self, probability: u32) -> BitResult<bool> {
        let mut value: u64 = self.value;
        let mut range: u32 = self.range;
        let mut bit_count: i32 = self.bit_count;

        let Some(chunk) = self.chunks.get(self.chunk_index).copied() else {
            return self.cold_read_bit_from_final_bytes(probability);
        };

        if self.bit_count < 0 {
            let v = u32::from_be_bytes(chunk);
            self.chunk_index += 1;
            value <<= 32;
            value |= u64::from(v);
            bit_count += 32;
        }
        debug_assert!(bit_count >= 0);

        let split = 1 + (((range - 1) * probability) >> 8);
        let bigsplit = u64::from(split) << bit_count;

        let retval = if let Some(new_value) = value.checked_sub(bigsplit) {
            range -= split;
            value = new_value;
            true
        } else {
            range = split;
            false
        };
        debug_assert!(range > 0);

        // Compute shift required to satisfy `range >= 128`.
        // Apply that shift to `range` and `self.bitcount`.
        //
        // Subtract 24 because we only care about leading zeros in the
        // lowest byte of `range` which is a `u32`.
        let shift = range.leading_zeros().saturating_sub(24);
        range <<= shift;
        bit_count -= shift as i32;
        debug_assert!(range >= 128);

        self.value = value;
        self.range = range;
        self.bit_count = bit_count;
        BitResult::ok(retval)
    }

    pub(crate) fn read_bool(&mut self, probability: u8) -> BitResult<bool> {
        self.read_bit(u32::from(probability))
    }

    pub(crate) fn read_literal(&mut self, n: u8) -> BitResult<u8> {
        let mut v = 0u8;
        let mut res = BitResult::OK;

        for _ in 0..n {
            let b = self.read_bit(128).or_accumulate(&mut res);
            v = (v << 1) + b as u8;
        }

        self.accumulated(res, v)
    }

    pub(crate) fn read_magnitude_and_sign(&mut self, n: u8) -> BitResult<i32> {
        let mut res = BitResult::OK;
        let magnitude = self.read_literal(n).or_accumulate(&mut res);
        let sign = self.read_flag().or_accumulate(&mut res);

        let value = if sign {
            -i32::from(magnitude)
        } else {
            i32::from(magnitude)
        };
        self.accumulated(res, value)
    }

    pub(crate) fn read_with_tree(
        &mut self,
        tree: &[i8],
        probs: &[u8],
        start: usize,
    ) -> BitResult<i8> {
        assert_eq!(probs.len() * 2, tree.len());
        assert!(start + 1 < tree.len());
        let mut index = start;
        let mut res = BitResult::OK;

        loop {
            let prob = probs[index as usize >> 1];
            let prob = u32::from(prob);
            let b = self.read_bit(prob).or_accumulate(&mut res);
            if b {
                index += 1;
            }
            let t = tree[index];
            if t > 0 {
                index = t as usize;
            } else {
                return self.accumulated(res, -t);
            }
        }
    }

    pub(crate) fn read_flag(&mut self) -> BitResult<bool> {
        self.read_bit(128)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_reader_hello_short() {
        let mut reader = BoolReader::new();
        let data = b"hel";
        let size = data.len();
        let mut buf = vec![[0u8; 4]; 1];
        buf.as_mut_slice().as_flattened_mut()[..size].copy_from_slice(&data[..]);
        reader.init(buf, size).unwrap();
        let mut res = BitResult::OK;
        assert_eq!(false, reader.read_bool(128).or_accumulate(&mut res));
        assert_eq!(true, reader.read_bool(10).or_accumulate(&mut res));
        assert_eq!(false, reader.read_bool(250).or_accumulate(&mut res));
        assert_eq!(1, reader.read_literal(1).or_accumulate(&mut res));
        assert_eq!(5, reader.read_literal(3).or_accumulate(&mut res));
        assert_eq!(64, reader.read_literal(8).or_accumulate(&mut res));
        assert_eq!(185, reader.read_literal(8).or_accumulate(&mut res));
        reader.check(res, ()).unwrap();
    }

    #[test]
    fn test_bool_reader_hello_long() {
        let mut reader = BoolReader::new();
        let data = b"hello world";
        let size = data.len();
        let mut buf = vec![[0u8; 4]; (size + 3) / 4];
        buf.as_mut_slice().as_flattened_mut()[..size].copy_from_slice(&data[..]);
        reader.init(buf, size).unwrap();
        let mut res = BitResult::OK;
        assert_eq!(false, reader.read_bool(128).or_accumulate(&mut res));
        assert_eq!(true, reader.read_bool(10).or_accumulate(&mut res));
        assert_eq!(false, reader.read_bool(250).or_accumulate(&mut res));
        assert_eq!(1, reader.read_literal(1).or_accumulate(&mut res));
        assert_eq!(5, reader.read_literal(3).or_accumulate(&mut res));
        assert_eq!(64, reader.read_literal(8).or_accumulate(&mut res));
        assert_eq!(185, reader.read_literal(8).or_accumulate(&mut res));
        assert_eq!(31, reader.read_literal(8).or_accumulate(&mut res));
        reader.check(res, ()).unwrap();
    }
}

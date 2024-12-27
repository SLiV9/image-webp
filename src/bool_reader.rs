use crate::decoder::DecodingError;

use super::vp8::TreeNode;

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
    state: State,
    final_bytes: [u8; 3],
    final_bytes_remaining: i8,
}

#[cfg_attr(test, derive(Debug))]
#[derive(Clone, Copy)]
struct State {
    chunk_index: usize,
    value: u64,
    range: u32,
    bit_count: i32,
}

#[cfg_attr(test, derive(Debug))]
struct FastReader<'a> {
    chunks: &'a [[u8; 4]],
    uncommitted_state: State,
    save_state: &'a mut State,
}

impl BoolReader {
    pub(crate) fn new() -> BoolReader {
        let state = State {
            chunk_index: 0,
            value: 0,
            range: 255,
            bit_count: -8,
        };
        BoolReader {
            chunks: Box::new([]),
            state,
            final_bytes: [0; 3],
            final_bytes_remaining: Self::FINAL_BYTES_REMAINING_EOF,
        }
    }

    pub(crate) fn init(&mut self, mut buf: Vec<[u8; 4]>, len: usize) -> Result<(), DecodingError> {
        let mut final_bytes = [0; 3];
        let final_bytes_remaining = if len == 4 * buf.len() {
            0
        } else {
            // Pop the last chunk (which is partial), then get length.
            let Some(last_chunk) = buf.pop() else {
                return Err(DecodingError::NotEnoughInitData);
            };
            let len_rounded_down = 4 * buf.len();
            let num_bytes_popped = len - len_rounded_down;
            debug_assert!(num_bytes_popped <= 3);
            for i in 0..num_bytes_popped {
                final_bytes[i] = last_chunk[i];
            }
            for i in num_bytes_popped..4 {
                debug_assert_eq!(last_chunk[i], 0, "unexpected {last_chunk:?}");
            }
            num_bytes_popped as i8
        };

        let chunks = buf.into_boxed_slice();
        let state = State {
            chunk_index: 0,
            value: 0,
            range: 255,
            bit_count: -8,
        };
        *self = Self {
            chunks,
            state,
            final_bytes,
            final_bytes_remaining,
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

    const FINAL_BYTES_REMAINING_EOF: i8 = -0xE;

    #[cold]
    fn load_final_bytes(&mut self) {
        if self.final_bytes_remaining > 0 {
            self.final_bytes_remaining -= 1;
            let byte = self.final_bytes[0];
            self.final_bytes.rotate_left(1);
            self.state.value <<= 8;
            self.state.value |= u64::from(byte);
            self.state.bit_count += 8;
        } else if self.final_bytes_remaining == 0 {
            // libwebp seems to (sometimes?) allow bitstreams that read one byte past the end.
            // This replicates that logic.
            self.final_bytes_remaining -= 1;
            self.state.value <<= 8;
            self.state.bit_count += 8;
        } else {
            self.final_bytes_remaining = Self::FINAL_BYTES_REMAINING_EOF;
        }
    }

    fn is_past_eof(&self) -> bool {
        self.final_bytes_remaining == Self::FINAL_BYTES_REMAINING_EOF
    }

    #[cold]
    fn cold_read_bit(&mut self, probability: u32) -> BitResult<bool> {
        if self.state.bit_count < 0 {
            if let Some(chunk) = self.chunks.get(self.state.chunk_index).copied() {
                let v = u32::from_be_bytes(chunk);
                self.state.chunk_index += 1;
                self.state.value <<= 32;
                self.state.value |= u64::from(v);
                self.state.bit_count += 32;
            } else {
                self.load_final_bytes();
                if self.is_past_eof() {
                    return BitResult::err();
                }
            }
        }
        debug_assert!(self.state.bit_count >= 0);

        let split = 1 + (((self.state.range - 1) * probability) >> 8);
        let bigsplit = u64::from(split) << self.state.bit_count;

        let retval = if let Some(new_value) = self.state.value.checked_sub(bigsplit) {
            self.state.range -= split;
            self.state.value = new_value;
            true
        } else {
            self.state.range = split;
            false
        };
        debug_assert!(self.state.range > 0);

        // Compute shift required to satisfy `self.state.range >= 128`.
        // Apply that shift to `self.state.range` and `self.state.bitcount`.
        //
        // Subtract 24 because we only care about leading zeros in the
        // lowest byte of `self.state.range` which is a `u32`.
        let shift = self.state.range.leading_zeros().saturating_sub(24);
        self.state.range <<= shift;
        self.state.bit_count -= shift as i32;
        debug_assert!(self.state.range >= 128);

        BitResult::ok(retval)
    }

    fn fast<'a>(&'a mut self) -> FastReader<'a> {
        FastReader {
            chunks: &self.chunks,
            uncommitted_state: self.state,
            save_state: &mut self.state,
        }
    }

    pub(crate) fn read_bool(&mut self, probability: u8) -> BitResult<bool> {
        let probability = u32::from(probability);

        if let Some(b) = self.fast().read_bit(probability) {
            return BitResult::ok(b);
        }

        self.cold_read_bool(probability)
    }

    #[cold]
    #[inline(never)]
    fn cold_read_bool(&mut self, probability: u32) -> BitResult<bool> {
        self.cold_read_bit(probability)
    }

    pub(crate) fn read_literal(&mut self, n: u8) -> BitResult<u8> {
        if let Some(v) = self.fast().read_literal(n) {
            return BitResult::ok(v);
        }

        self.cold_read_literal(n)
    }

    #[cold]
    #[inline(never)]
    fn cold_read_literal(&mut self, n: u8) -> BitResult<u8> {
        let mut v = 0u8;
        let mut res = BitResult::OK;

        for _ in 0..n {
            let b = self.cold_read_bit(128).or_accumulate(&mut res);
            v = (v << 1) + b as u8;
        }

        self.accumulated(res, v)
    }

    pub(crate) fn read_magnitude_and_sign(&mut self, n: u8) -> BitResult<i32> {
        if let Some(v) = self.fast().read_magnitude_and_sign(n) {
            return BitResult::ok(v);
        }

        self.cold_read_magnitude_and_sign(n)
    }

    #[cold]
    #[inline(never)]
    fn cold_read_magnitude_and_sign(&mut self, n: u8) -> BitResult<i32> {
        let mut res = BitResult::OK;
        let magnitude = self.cold_read_literal(n).or_accumulate(&mut res);
        let sign = self.cold_read_bool(128).or_accumulate(&mut res);

        let value = if sign {
            -i32::from(magnitude)
        } else {
            i32::from(magnitude)
        };
        self.accumulated(res, value)
    }

    pub(crate) fn read_with_tree(&mut self, tree: &[TreeNode], skip: bool) -> BitResult<i8> {
        if let Some(v) = self.fast().read_with_tree(tree, skip) {
            return BitResult::ok(v);
        }

        self.cold_read_with_tree(tree, skip)
    }

    #[cold]
    fn cold_read_with_tree(&mut self, tree: &[TreeNode], skip: bool) -> BitResult<i8> {
        let mut index = skip as usize;
        let mut res = BitResult::OK;

        loop {
            let node = tree[index];
            let prob = u32::from(node.prob);
            let b = self.cold_read_bit(prob).or_accumulate(&mut res);
            let t = if b { node.right } else { node.left };
            let new_index = usize::from(t);
            if new_index < tree.len() {
                index = new_index;
            } else {
                let value = TreeNode::value_from_branch(t);
                return self.accumulated(res, value);
            }
        }
    }

    pub(crate) fn read_flag(&mut self) -> BitResult<bool> {
        self.read_bool(128)
    }
}

impl<'a> FastReader<'a> {
    fn commit_if_valid<T>(self, acc: BitResultAccumulator, value_if_not_past_eof: T) -> Option<T> {
        let _ = acc;
        if self.uncommitted_state.chunk_index < self.chunks.len() {
            *self.save_state = self.uncommitted_state;
            Some(value_if_not_past_eof)
        } else {
            None
        }
    }

    fn read_bit(mut self, probability: u32) -> Option<bool> {
        let mut res = BitResult::OK;
        let b = self.fast_read_bit(probability, &mut res);
        self.commit_if_valid(res, b)
    }

    fn read_literal(mut self, n: u8) -> Option<u8> {
        let mut res = BitResult::OK;
        let b = self.fast_read_literal(n, &mut res);
        self.commit_if_valid(res, b)
    }

    fn read_magnitude_and_sign(mut self, n: u8) -> Option<i32> {
        let mut res = BitResult::OK;
        let b = self.fast_read_magnitude_and_sign(n, &mut res);
        self.commit_if_valid(res, b)
    }

    fn read_with_tree(mut self, tree: &[TreeNode], skip: bool) -> Option<i8> {
        let mut res = BitResult::OK;
        let b = self.fast_read_with_tree(tree, skip, &mut res);
        self.commit_if_valid(res, b)
    }

    fn fast_read_bit(&mut self, probability: u32, acc: &mut BitResultAccumulator) -> bool {
        let State {
            mut chunk_index,
            mut value,
            mut range,
            mut bit_count,
        } = self.uncommitted_state;

        if bit_count < 0 {
            let chunk = match self.chunks.get(chunk_index).copied() {
                Some(chunk) => BitResult::ok(chunk),
                None => BitResult::err(),
            };
            let chunk = chunk.or_accumulate(acc);
            let v = u32::from_be_bytes(chunk);
            chunk_index += 1;
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

        self.uncommitted_state = State {
            chunk_index,
            value,
            range,
            bit_count,
        };
        retval
    }

    fn fast_read_literal(&mut self, n: u8, acc: &mut BitResultAccumulator) -> u8 {
        let mut v = 0u8;
        for _ in 0..n {
            let b = self.fast_read_bit(128, acc);
            v = (v << 1) + b as u8;
        }
        v
    }

    fn fast_read_magnitude_and_sign(&mut self, n: u8, acc: &mut BitResultAccumulator) -> i32 {
        let magnitude = self.fast_read_literal(n, acc);
        let sign = self.fast_read_bit(128, acc);
        if sign {
            -i32::from(magnitude)
        } else {
            i32::from(magnitude)
        }
    }

    fn fast_read_with_tree(
        &mut self,
        tree: &[TreeNode],
        skip: bool,
        acc: &mut BitResultAccumulator,
    ) -> i8 {
        let mut i = skip as u8;
        while let Some(node) = tree.get(usize::from(i)) {
            let prob = u32::from(node.prob);
            let b = self.fast_read_bit(prob, acc);
            i = if b { node.right } else { node.left };
        }
        TreeNode::value_from_branch(i)
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

    #[test]
    fn test_bool_reader_uninit() {
        let mut reader = BoolReader::new();
        let mut res = BitResult::OK;
        let _ = reader.read_flag().or_accumulate(&mut res);
        let result = reader.check(res, ());
        assert!(result.is_err());
    }
}

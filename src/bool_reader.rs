use crate::decoder::DecodingError;

use super::vp8::TreeNode;

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
            final_bytes_remaining: -1,
        }
    }

    pub(crate) fn init(&mut self, mut buf: Vec<[u8; 4]>, len: usize) -> Result<(), DecodingError> {
        let mut final_bytes = [0; 3];
        let final_bytes_remaining = if len == 4 * buf.len() {
            0
        } else {
            debug_assert!(len < 4 * buf.len());
            // Pop the last chunk (which is partial), then get length.
            let last_chunk = buf.pop().unwrap();
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

    fn fast<'a>(&'a mut self) -> FastReader<'a> {
        FastReader {
            chunks: &self.chunks,
            uncommitted_state: self.state,
            save_state: &mut self.state,
        }
    }

    #[cold]
    fn cold_read_bit(&mut self, probability: u8) -> Result<bool, DecodingError> {
        if self.state.bit_count < 0 {
            if let Some(chunk) = self.chunks.get(self.state.chunk_index).copied() {
                let v = u32::from_be_bytes(chunk);
                self.state.chunk_index += 1;
                self.state.value <<= 32;
                self.state.value |= u64::from(v);
                self.state.bit_count += 32;
            } else {
                self.load_final_bytes()?;
            }
        }
        debug_assert!(self.state.bit_count >= 0);

        let probability = u32::from(probability);
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

        Ok(retval)
    }

    #[cold]
    fn load_final_bytes(&mut self) -> Result<(), DecodingError> {
        if self.final_bytes_remaining > 0 {
            self.final_bytes_remaining -= 1;
            let byte = self.final_bytes[0];
            self.final_bytes.rotate_left(1);
            self.state.value <<= 8;
            self.state.value |= u64::from(byte);
            self.state.bit_count += 8;
            Ok(())
        } else if self.final_bytes_remaining == 0 {
            // libwebp seems to (sometimes?) allow bitstreams that read one byte past the end.
            // This replicates that logic.
            self.final_bytes_remaining -= 1;
            self.state.value <<= 8;
            self.state.bit_count += 8;
            Ok(())
        } else {
            Err(DecodingError::BitStreamError)
        }
    }

    #[inline(never)]
    pub(crate) fn read_bool(&mut self, probability: u8) -> Result<bool, DecodingError> {
        if let Some(b) = self.fast().read_bit(probability) {
            return Ok(b);
        }

        self.cold_read_bool(probability)
    }

    #[cold]
    #[inline(never)]
    fn cold_read_bool(&mut self, probability: u8) -> Result<bool, DecodingError> {
        self.cold_read_bit(probability)
    }

    #[inline(never)]
    pub(crate) fn read_literal(&mut self, n: u8) -> Result<u8, DecodingError> {
        if let Some(v) = self.fast().read_literal(n) {
            return Ok(v);
        }

        self.cold_read_literal(n)
    }

    #[cold]
    #[inline(never)]
    fn cold_read_literal(&mut self, n: u8) -> Result<u8, DecodingError> {
        let mut v = 0u8;

        for _ in 0..n {
            let b = self.cold_read_bit(128)?;
            v = (v << 1) + b as u8;
        }

        Ok(v)
    }

    #[inline(never)]
    pub(crate) fn read_optional_signed_value(&mut self, n: u8) -> Result<i32, DecodingError> {
        if let Some(v) = self.fast().read_optional_signed_value(n) {
            return Ok(v);
        }

        self.cold_read_optional_signed_value(n)
    }

    #[cold]
    #[inline(never)]
    fn cold_read_optional_signed_value(&mut self, n: u8) -> Result<i32, DecodingError> {
        let flag = self.cold_read_bool(128)?;
        if !flag {
            // We should not read further bits if the flag is not set.
            return Ok(0);
        }
        let magnitude = self.cold_read_literal(n)?;
        let sign = self.cold_read_bool(128)?;

        let value = if sign {
            -i32::from(magnitude)
        } else {
            i32::from(magnitude)
        };
        Ok(value)
    }

    #[inline]
    pub(crate) fn read_with_tree<const N: usize>(
        &mut self,
        tree: &[TreeNode; N],
    ) -> Result<i8, DecodingError> {
        let first_node = tree[0];
        self.read_with_tree_with_first_node(tree, first_node)
    }

    #[inline(never)]
    pub(crate) fn read_with_tree_with_first_node(
        &mut self,
        tree: &[TreeNode],
        first_node: TreeNode,
    ) -> Result<i8, DecodingError> {
        if let Some(v) = self.fast().read_with_tree(tree, first_node) {
            return Ok(v);
        }

        self.cold_read_with_tree(tree, usize::from(first_node.index))
    }

    #[cold]
    fn cold_read_with_tree(
        &mut self,
        tree: &[TreeNode],
        start: usize,
    ) -> Result<i8, DecodingError> {
        let mut index = start;

        loop {
            let node = tree[index];
            let prob = node.prob;
            let b = self.cold_read_bit(prob)?;
            let t = if b { node.right } else { node.left };
            let new_index = usize::from(t);
            if new_index < tree.len() {
                index = new_index;
            } else {
                let value = TreeNode::value_from_branch(t);
                return Ok(value);
            }
        }
    }

    #[inline]
    pub(crate) fn read_flag(&mut self) -> Result<bool, DecodingError> {
        self.read_bool(128)
    }
}

impl<'a> FastReader<'a> {
    fn commit_if_valid<T>(self, value_if_not_past_eof: T) -> Option<T> {
        // If `chunk_index > self.chunks.len()`, it means we used zeroes
        // instead of an actual chunk and `value_if_not_past_eof` is nonsense.
        if self.uncommitted_state.chunk_index <= self.chunks.len() {
            *self.save_state = self.uncommitted_state;
            Some(value_if_not_past_eof)
        } else {
            None
        }
    }

    fn read_bit(mut self, probability: u8) -> Option<bool> {
        let bit = self.fast_read_bit(probability);
        self.commit_if_valid(bit)
    }

    fn read_literal(mut self, n: u8) -> Option<u8> {
        let value = self.fast_read_literal(n);
        self.commit_if_valid(value)
    }

    fn read_optional_signed_value(mut self, n: u8) -> Option<i32> {
        let flag = self.fast_read_bit(128);
        if !flag {
            // We should not read further bits if the flag is not set.
            return self.commit_if_valid(0);
        }
        let magnitude = self.fast_read_literal(n);
        let sign = self.fast_read_bit(128);
        let value = if sign {
            -i32::from(magnitude)
        } else {
            i32::from(magnitude)
        };
        self.commit_if_valid(value)
    }

    fn read_with_tree(mut self, tree: &[TreeNode], first_node: TreeNode) -> Option<i8> {
        let value = self.fast_read_with_tree(tree, first_node);
        self.commit_if_valid(value)
    }

    fn fast_read_bit(&mut self, probability: u8) -> bool {
        let State {
            mut chunk_index,
            mut value,
            mut range,
            mut bit_count,
        } = self.uncommitted_state;

        if bit_count < 0 {
            let chunk = self.chunks.get(chunk_index).copied();
            // We ignore invalid data inside the `fast_` functions,
            // but we increase `chunk_index` below, so we can check
            // whether we read invalid data in `commit_if_valid`.
            let chunk = chunk.unwrap_or_default();

            let v = u32::from_be_bytes(chunk);
            chunk_index += 1;
            value <<= 32;
            value |= u64::from(v);
            bit_count += 32;
        }
        debug_assert!(bit_count >= 0);

        let probability = u32::from(probability);
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

    fn fast_read_literal(&mut self, n: u8) -> u8 {
        let mut v = 0u8;
        for _ in 0..n {
            let b = self.fast_read_bit(128);
            v = (v << 1) + b as u8;
        }
        v
    }

    fn fast_read_with_tree(&mut self, tree: &[TreeNode], mut node: TreeNode) -> i8 {
        loop {
            let prob = node.prob;
            let b = self.fast_read_bit(prob);
            let i = if b { node.right } else { node.left };
            let Some(next_node) = tree.get(usize::from(i)) else {
                return TreeNode::value_from_branch(i);
            };
            node = *next_node;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_reader_hello_short() -> Result<(), DecodingError> {
        let mut reader = BoolReader::new();
        let data = b"hel";
        let size = data.len();
        let mut buf = vec![[0u8; 4]; 1];
        buf.as_mut_slice().as_flattened_mut()[..size].copy_from_slice(&data[..]);
        reader.init(buf, size).unwrap();
        assert_eq!(false, reader.read_bool(128)?);
        assert_eq!(true, reader.read_bool(10)?);
        assert_eq!(false, reader.read_bool(250)?);
        assert_eq!(1, reader.read_literal(1)?);
        assert_eq!(5, reader.read_literal(3)?);
        assert_eq!(64, reader.read_literal(8)?);
        assert_eq!(185, reader.read_literal(8)?);
        Ok(())
    }

    #[test]
    fn test_bool_reader_hello_long() -> Result<(), DecodingError> {
        let mut reader = BoolReader::new();
        let data = b"hello world";
        let size = data.len();
        let mut buf = vec![[0u8; 4]; (size + 3) / 4];
        buf.as_mut_slice().as_flattened_mut()[..size].copy_from_slice(&data[..]);
        reader.init(buf, size).unwrap();
        assert_eq!(false, reader.read_bool(128)?);
        assert_eq!(true, reader.read_bool(10)?);
        assert_eq!(false, reader.read_bool(250)?);
        assert_eq!(1, reader.read_literal(1)?);
        assert_eq!(5, reader.read_literal(3)?);
        assert_eq!(64, reader.read_literal(8)?);
        assert_eq!(185, reader.read_literal(8)?);
        assert_eq!(31, reader.read_literal(8)?);
        Ok(())
    }

    #[test]
    fn test_bool_reader_uninit() {
        let mut reader = BoolReader::new();
        assert!(reader.read_flag().is_err());
    }
}

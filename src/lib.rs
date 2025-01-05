//! Decoding and Encoding of WebP Images

#![forbid(unsafe_code)]
#![deny(missing_docs)]
// Increase recursion limit for the `quick_error!` macro.
#![recursion_limit = "256"]
// Enable nightly benchmark functionality if "_benchmarks" feature is enabled.
#![cfg_attr(all(test, feature = "_benchmarks"), feature(test))]
#[cfg(all(test, feature = "_benchmarks"))]
extern crate test;

pub use self::decoder::{DecodingError, LoopCount, WebPDecoder};
pub use self::encoder::{ColorType, EncoderParams, EncodingError, WebPEncoder};

mod alpha_blending;
mod decoder;
mod encoder;
mod extended;
mod huffman;
mod loop_filter;
mod lossless;
mod lossless_transform;
mod transform;
mod vp8_arithmetic_decoder;

pub mod vp8;

#[inline(never)]
/// TODO remove
pub fn foo(xrange: u64, probability: u8) -> u64 {
    let probability = u64::from(probability);
    let bsr = xrange.leading_zeros() as i32 - 32;
    let bit_count = 24 - bsr;
    let range = xrange >> bit_count;
    debug_assert!(range <= 0xFF);
    let split = 1 + (((range - 1) * probability) >> 8);
    let bigsplit = u64::from(split) << bit_count;

    bigsplit
}

#[inline(never)]
/// TODO remove
pub fn bar(xrange: u64, probability: u8) -> u64 {
    debug_assert!(xrange.leading_zeros() <= 56);
    debug_assert!(xrange.leading_zeros() >= 24);
    let probability = u64::from(probability);
    let s = xrange.leading_zeros() - 24;
    let mut r = xrange << s;
    debug_assert_eq!(r.leading_zeros(), 24);
    r &= 0x000000FF00000000;
    r -= 0x0000000100000000;
    r *= probability;
    r &= 0x0000FF0000000000;
    r += 0x0000010000000000;
    r >>= s + 8;

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foo_bar() {
        let mut xrange = 0x000000017b;
        for i in 0..20 {
            for prob in 0..256 {
                let a = foo(xrange, prob as u8);
                let b = bar(xrange, prob as u8);
                let bsr = xrange.leading_zeros() as i32 - 32;
                let bit_count = 24 - bsr;
                let range = (xrange >> bit_count) & 0xFF;
                if a != b {
                    eprintln!("xrange={xrange:#042b} bsr={bsr} bit_count={bit_count} range={range} prob={prob} a={a} b={b}");
                }
                assert_eq!(a, b);
            }
            xrange <<= 1;
            xrange |= ((i * 13) % 17) & 0b1;
        }
    }
}

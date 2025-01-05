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
    let bsr = xrange.leading_zeros();
    let bit_count = 56 - bsr;
    let range = (xrange >> bit_count) as u16;
    debug_assert!(range <= 0xFF);
    let probability = u16::from(probability);
    let x = 0x0100 + ((range - 1) * probability);
    let [_, split] = x.to_le_bytes();
    let bigsplit = u64::from(split) << bit_count;

    bigsplit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foo_bar() {
        let xrange = 0x000000017b;
        for prob in 0..=255 {
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
    }
}

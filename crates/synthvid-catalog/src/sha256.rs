//! SHA-256 hash function.
//!
//! Implemented from FIPS 180-4 "Secure Hash Standard (SHS)".

/// The digest size of SHA-256 in bytes.
pub const DIGEST_SIZE: usize = 32;

/// A SHA-256 hash digest.
///
/// This is a newtype wrapper around a 32-byte array that provides a
/// lowercase hexadecimal `Display` implementation and protects against
/// accidental type confusion with other 32-byte values.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Digest([u8; 32]);

impl Digest {
    /// Construct a `Digest` from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Access the raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl core::fmt::Display for Digest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl core::fmt::Debug for Digest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Digest")
            .field(&format_args!("{self}"))
            .finish()
    }
}

/// SHA-256 hash state.
#[derive(Clone, Copy, Debug)]
struct State {
    /// First hash state variable (h0).
    h0: u32,
    /// Second hash state variable (h1).
    h1: u32,
    /// Third hash state variable (h2).
    h2: u32,
    /// Fourth hash state variable (h3).
    h3: u32,
    /// Fifth hash state variable (h4).
    h4: u32,
    /// Sixth hash state variable (h5).
    h5: u32,
    /// Seventh hash state variable (h6).
    h6: u32,
    /// Eighth hash state variable (h7).
    h7: u32,
}

impl State {
    /// Initialize a new SHA-256 state with the standard initial hash values.
    const fn new() -> Self {
        Self {
            h0: 0x6a09_e667u32,
            h1: 0xbb67_ae85u32,
            h2: 0x3c6e_f372u32,
            h3: 0xa54f_f53au32,
            h4: 0x510e_527fu32,
            h5: 0x9b05_688cu32,
            h6: 0x1f83_d9abu32,
            h7: 0x5be0_cd19u32,
        }
    }
}

/// A streaming SHA-256 hasher.
///
/// Allows incremental hashing of data as it becomes available, avoiding the
/// need to buffer entire inputs in memory.
#[derive(Clone, Copy, Debug)]
pub struct Sha256 {
    /// Current hash state.
    state: State,
    /// Partial block buffer.
    buffer: [u8; 64],
    /// Number of bytes in the buffer.
    buffer_len: usize,
    /// Total bytes processed (before buffering).
    total_len: u64,
}

impl Sha256 {
    /// Create a new SHA-256 hasher.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: State::new(),
            buffer: [0u8; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    /// Update the hasher with new data.
    ///
    /// This method can be called multiple times with different data.
    /// The hasher will process complete 64-byte blocks immediately and
    /// buffer any partial block until [`finalize`](Self::finalize) is called.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The data to hash.
    pub fn update(&mut self, bytes: &[u8]) {
        self.total_len = self
            .total_len
            .wrapping_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        let mut offset = 0;
        if self.buffer_len > 0 {
            let available = 64usize.saturating_sub(self.buffer_len);
            let to_copy = available.min(bytes.len());
            if let Some(buf_slice) = self
                .buffer
                .get_mut(self.buffer_len..self.buffer_len.saturating_add(to_copy))
            {
                if let Some(bytes_slice) = bytes.get(..to_copy) {
                    buf_slice.copy_from_slice(bytes_slice);
                }
            }
            self.buffer_len = self.buffer_len.saturating_add(to_copy);
            offset = to_copy;
            if self.buffer_len == 64 {
                process_block(&self.buffer, &mut self.state);
                self.buffer_len = 0;
            } else {
                return;
            }
        }
        if let Some(remaining) = bytes.get(offset..) {
            let (chunks, remainder) = remaining.as_chunks::<64>();
            for chunk in chunks {
                process_block(chunk, &mut self.state);
            }
            self.buffer_len = remainder.len();
            if self.buffer_len > 0 {
                if let Some(buf_slice) = self.buffer.get_mut(..self.buffer_len) {
                    buf_slice.copy_from_slice(remainder);
                }
            }
        }
    }

    /// Finalize the hasher and return the digest.
    #[must_use]
    pub fn finalize(mut self) -> Digest {
        let mut last = [0u8; 64];
        if let Some(dst) = last.get_mut(..self.buffer_len) {
            if let Some(src) = self.buffer.get(..self.buffer_len) {
                dst.copy_from_slice(src);
            }
        }
        if let Some(marker) = last.get_mut(self.buffer_len) {
            *marker = 0x80;
        }
        let bit_len = self.total_len.wrapping_mul(8);
        if self.buffer_len >= 56 {
            process_block(&last, &mut self.state);
            last = [0u8; 64];
        }
        if let Some(dst) = last.get_mut(56..64) {
            dst.copy_from_slice(&bit_len.to_be_bytes());
        }
        process_block(&last, &mut self.state);
        let mut result = [0u8; 32];
        let (output_chunks, _) = result.as_chunks_mut::<4>();
        for (chunk, word) in output_chunks.iter_mut().zip([
            self.state.h0,
            self.state.h1,
            self.state.h2,
            self.state.h3,
            self.state.h4,
            self.state.h5,
            self.state.h6,
            self.state.h7,
        ]) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        Digest::new(result)
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

/// SHA-256 hash function.
///
/// # Arguments
///
/// * `input` - The message to hash.
///
/// # Returns
///
/// A SHA-256 digest.
///
/// This is a convenience function that creates a [`Sha256`] hasher,
/// feeds it the entire input, and returns the result. For very large inputs
/// or when data is not available all at once, use [`Sha256`] directly.
#[must_use]
pub fn sha256(input: &[u8]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(input);
    hasher.finalize()
}

/// Wrapper type for SHA-256 words with wrapping arithmetic.
///
/// `+` is deliberately not implemented: `clippy::arithmetic_side_effects`
/// flags any use of the `+` operator, including through a custom `Add`
/// impl, so wrapping addition is exposed as the [`W::wrapping_add`] method
/// instead -- the same way `u32::wrapping_add` itself is clippy-clean.
#[derive(Clone, Copy, Default)]
struct W(u32);

impl core::ops::BitXor for W {
    type Output = Self;

    fn bitxor(self, other: Self) -> Self {
        Self(self.0 ^ other.0)
    }
}

impl core::ops::BitAnd for W {
    type Output = Self;

    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

impl core::ops::Not for W {
    type Output = Self;

    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl W {
    /// Rotate right by n bits.
    const fn rotate_right(self, n: u32) -> Self {
        Self(self.0.rotate_right(n))
    }

    /// Add, wrapping on overflow.
    const fn wrapping_add(self, other: Self) -> Self {
        Self(self.0.wrapping_add(other.0))
    }
}

/// Round constants (first 32 bits of the fractional parts of the cube
/// roots of the first 64 primes), per FIPS 180-4.
const K: [W; 64] = [
    W(0x428a_2f98),
    W(0x7137_4491),
    W(0xb5c0_fbcf),
    W(0xe9b5_dba5),
    W(0x3956_c25b),
    W(0x59f1_11f1),
    W(0x923f_82a4),
    W(0xab1c_5ed5),
    W(0xd807_aa98),
    W(0x1283_5b01),
    W(0x2431_85be),
    W(0x550c_7dc3),
    W(0x72be_5d74),
    W(0x80de_b1fe),
    W(0x9bdc_06a7),
    W(0xc19b_f174),
    W(0xe49b_69c1),
    W(0xefbe_4786),
    W(0x0fc1_9dc6),
    W(0x240c_a1cc),
    W(0x2de9_2c6f),
    W(0x4a74_84aa),
    W(0x5cb0_a9dc),
    W(0x76f9_88da),
    W(0x983e_5152),
    W(0xa831_c66d),
    W(0xb003_27c8),
    W(0xbf59_7fc7),
    W(0xc6e0_0bf3),
    W(0xd5a7_9147),
    W(0x06ca_6351),
    W(0x1429_2967),
    W(0x27b7_0a85),
    W(0x2e1b_2138),
    W(0x4d2c_6dfc),
    W(0x5338_0d13),
    W(0x650a_7354),
    W(0x766a_0abb),
    W(0x81c2_c92e),
    W(0x9272_2c85),
    W(0xa2bf_e8a1),
    W(0xa81a_664b),
    W(0xc24b_8b70),
    W(0xc76c_51a3),
    W(0xd192_e819),
    W(0xd699_0624),
    W(0xf40e_3585),
    W(0x106a_a070),
    W(0x19a4_c116),
    W(0x1e37_6c08),
    W(0x2748_774c),
    W(0x34b0_bcb5),
    W(0x391c_0cb3),
    W(0x4ed8_aa4a),
    W(0x5b9c_ca4f),
    W(0x682e_6ff3),
    W(0x748f_82ee),
    W(0x78a5_636f),
    W(0x84c8_7814),
    W(0x8cc7_0208),
    W(0x90be_fffa),
    W(0xa450_6ceb),
    W(0xbef9_a3f7),
    W(0xc671_78f2),
];

/// The eight FIPS 180-4 working variables for one block's compression.
///
/// Held as struct fields rather than locals named `a` through `h`: clippy's
/// `many_single_char_names` counts single-character *bindings* in a scope,
/// not struct fields, so this keeps the FIPS notation (`working.a`, ...)
/// without needing a lint suppression.
struct Working {
    /// First working variable (a).
    a: W,
    /// Second working variable (b).
    b: W,
    /// Third working variable (c).
    c: W,
    /// Fourth working variable (d).
    d: W,
    /// Fifth working variable (e).
    e: W,
    /// Sixth working variable (f).
    f: W,
    /// Seventh working variable (g).
    g: W,
    /// Eighth working variable (h).
    h: W,
}

/// Process a single 512-bit (64-byte) SHA-256 message block.
///
/// The message schedule is expanded into a rolling 16-word window rather
/// than a full 64-word array: FIPS 180-4's backreferences `w[i-15]`,
/// `w[i-2]`, `w[i-16]`, and `w[i-7]` are always exactly 15, 2, 16, and 7
/// slots behind the word being produced, so against a window holding the
/// most recent 16 words they are always the fixed offsets 1, 14, 0, and 9.
/// `window.rotate_left(1)` slides the window forward each round, keeping
/// every access a literal index into a fixed-size array (which the compiler
/// itself proves in bounds), rather than an index computed from the round
/// counter.
fn process_block(block: &[u8; 64], state: &mut State) {
    // The rolling window starts as the first 16 message-schedule words,
    // read directly from the block.
    let mut window = [W(0); 16];
    let (chunks, _) = block.as_chunks::<4>();
    for (slot, chunk) in window.iter_mut().zip(chunks) {
        *slot = W(u32::from_be_bytes(*chunk));
    }

    // Initialize working variables
    let mut working = Working {
        a: W(state.h0),
        b: W(state.h1),
        c: W(state.h2),
        d: W(state.h3),
        e: W(state.h4),
        f: W(state.h5),
        g: W(state.h6),
        h: W(state.h7),
    };

    // One round of the compression function, given this round's schedule
    // word and round constant.
    let mut round = |k: W, w_i: W| {
        let sigma1 =
            working.e.rotate_right(6) ^ working.e.rotate_right(11) ^ working.e.rotate_right(25);
        let ch = (working.e & working.f) ^ ((!working.e) & working.g);
        let temp1 = working
            .h
            .wrapping_add(sigma1)
            .wrapping_add(ch)
            .wrapping_add(k)
            .wrapping_add(w_i);
        let sigma0 =
            working.a.rotate_right(2) ^ working.a.rotate_right(13) ^ working.a.rotate_right(22);
        let maj = (working.a & working.b) ^ (working.a & working.c) ^ (working.b & working.c);
        let temp2 = sigma0.wrapping_add(maj);

        working.h = working.g;
        working.g = working.f;
        working.f = working.e;
        working.e = working.d.wrapping_add(temp1);
        working.d = working.c;
        working.c = working.b;
        working.b = working.a;
        working.a = temp1.wrapping_add(temp2);
    };

    // Rounds 0..16 consume the schedule words as loaded from the block.
    for (&k, &w_i) in K[..16].iter().zip(window.iter()) {
        round(k, w_i);
    }

    // Rounds 16..64 expand one new schedule word per round from the
    // rolling window before consuming it; see the doc comment above for
    // why the backreferences become fixed offsets 0, 1, 9, and 14.
    for &k in &K[16..64] {
        let s0 = window[1].rotate_right(7) ^ window[1].rotate_right(18) ^ W(window[1].0 >> 3);
        let s1 = window[14].rotate_right(17) ^ window[14].rotate_right(19) ^ W(window[14].0 >> 10);
        let w_i = window[0]
            .wrapping_add(s0)
            .wrapping_add(window[9])
            .wrapping_add(s1);
        window.rotate_left(1);
        window[15] = w_i;

        round(k, w_i);
    }

    // Add compressed chunk to current hash value
    state.h0 = state.h0.wrapping_add(working.a.0);
    state.h1 = state.h1.wrapping_add(working.b.0);
    state.h2 = state.h2.wrapping_add(working.c.0);
    state.h3 = state.h3.wrapping_add(working.d.0);
    state.h4 = state.h4.wrapping_add(working.e.0);
    state.h5 = state.h5.wrapping_add(working.f.0);
    state.h6 = state.h6.wrapping_add(working.g.0);
    state.h7 = state.h7.wrapping_add(working.h.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let result = sha256(b"");
        let expected = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(result.as_bytes(), &expected);
    }

    #[test]
    fn test_abc() {
        let result = sha256(b"abc");
        let expected = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(result.as_bytes(), &expected);
    }

    #[test]
    fn test_long_input() {
        let input = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        let result = sha256(input);
        let expected = [
            0x24, 0x8d, 0x6a, 0x61, 0xd2, 0x06, 0x38, 0xb8, 0xe5, 0xc0, 0x26, 0x93, 0x0c, 0x3e,
            0x60, 0x39, 0xa3, 0x3c, 0xe4, 0x59, 0x64, 0xff, 0x21, 0x67, 0xf6, 0xec, 0xed, 0xd4,
            0x19, 0xdb, 0x06, 0xc1,
        ];
        assert_eq!(result.as_bytes(), &expected);
    }

    #[test]
    fn test_one_million_a() {
        let input = vec![b'a'; 1_000_000];
        let result = sha256(&input);
        let expected = [
            0xcd, 0xc7, 0x6e, 0x5c, 0x99, 0x14, 0xfb, 0x92, 0x81, 0xa1, 0xc7, 0xe2, 0x84, 0xd7,
            0x3e, 0x67, 0xf1, 0x80, 0x9a, 0x48, 0xa4, 0x97, 0x20, 0x0e, 0x04, 0x6d, 0x39, 0xcc,
            0xc7, 0x11, 0x2c, 0xd0,
        ];
        assert_eq!(result.as_bytes(), &expected);
    }

    /// Block-boundary length vectors, cross-checked against an independent
    /// oracle (Python's `hashlib.sha256`, a separate implementation in a
    /// separate language) rather than this workspace's own code.
    ///
    /// Inputs are generated with [`synthvid_scene::Rng`] (this workspace's
    /// existing deterministic `SplitMix64` generator) rather than a new
    /// hand-rolled one, so the oracle computation is reproducible offline;
    /// the crate itself performs no I/O; ambient process/network access is
    /// forbidden in this crate by `ci/check-forbidden-tokens.sh`, so the
    /// oracle comparison happens once, at authoring time, and its result is
    /// frozen here as a literal, the same way the FIPS 180-4 vectors above
    /// are.
    const VECTORS: [(usize, [u8; DIGEST_SIZE]); 15] = [
        (
            0,
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                0x78, 0x52, 0xb8, 0x55,
            ],
        ),
        (
            1,
            [
                0x65, 0xc7, 0x4c, 0x15, 0xa6, 0x86, 0x18, 0x7b, 0xb6, 0xbb, 0xf9, 0x95, 0x8f, 0x49,
                0x4f, 0xc6, 0xb8, 0x00, 0x68, 0x03, 0x4a, 0x65, 0x9a, 0x9a, 0xd4, 0x49, 0x91, 0xb0,
                0x8c, 0x58, 0xf2, 0xd2,
            ],
        ),
        (
            2,
            [
                0x07, 0xdf, 0x64, 0xa5, 0x5f, 0x51, 0x70, 0x5a, 0x16, 0x7d, 0xa3, 0xee, 0x40, 0xdd,
                0xdb, 0x0d, 0x8d, 0x91, 0xba, 0x42, 0xd7, 0x51, 0x79, 0xaf, 0x1f, 0xaa, 0x1e, 0x90,
                0xc2, 0x6c, 0x02, 0xac,
            ],
        ),
        (
            55,
            [
                0x04, 0x65, 0x9e, 0x8b, 0xd5, 0xd8, 0x59, 0xdf, 0x3e, 0xf1, 0xf5, 0xec, 0x72, 0xeb,
                0xa7, 0x21, 0x76, 0x29, 0x96, 0x95, 0xe2, 0xb0, 0xf5, 0x50, 0xd1, 0xa0, 0x87, 0x4e,
                0xde, 0x1a, 0x2f, 0x3c,
            ],
        ),
        (
            56,
            [
                0x4e, 0x41, 0xc3, 0x5a, 0x38, 0xcf, 0x07, 0x66, 0x70, 0xc8, 0xed, 0xf8, 0x7b, 0x79,
                0x8b, 0x3f, 0xff, 0x44, 0xea, 0xbb, 0xce, 0x19, 0xf5, 0x87, 0xb5, 0x52, 0x52, 0x22,
                0x1f, 0xbe, 0x15, 0xea,
            ],
        ),
        (
            57,
            [
                0x01, 0xa7, 0x8e, 0xa6, 0x63, 0xb0, 0xdc, 0xb2, 0x1f, 0x49, 0xbc, 0xcc, 0x87, 0xbc,
                0x26, 0x26, 0xe2, 0x7c, 0xf9, 0xd3, 0x3a, 0x13, 0x10, 0x99, 0x99, 0xf8, 0x7f, 0x82,
                0xc7, 0x45, 0xae, 0x71,
            ],
        ),
        (
            63,
            [
                0x1b, 0x00, 0xdc, 0xa2, 0x5c, 0x9a, 0x40, 0x7e, 0x7f, 0x1e, 0x8e, 0xd5, 0x82, 0xba,
                0x33, 0x37, 0x75, 0xde, 0x3e, 0xef, 0x63, 0x06, 0x92, 0x01, 0xa0, 0x8a, 0x9d, 0x4b,
                0xb1, 0xd8, 0xa1, 0xb7,
            ],
        ),
        (
            64,
            [
                0x51, 0x1a, 0xe7, 0x36, 0x07, 0x38, 0x72, 0x19, 0x86, 0x3f, 0x9d, 0x4a, 0xa6, 0xf2,
                0x2e, 0xbf, 0x2b, 0x8e, 0xf0, 0xe6, 0xd3, 0x7b, 0x6e, 0x51, 0xf6, 0x99, 0x88, 0xf8,
                0xab, 0x93, 0x51, 0x8c,
            ],
        ),
        (
            65,
            [
                0xc7, 0xa6, 0x73, 0x67, 0xc0, 0x61, 0x55, 0xef, 0xac, 0x77, 0x9a, 0x3f, 0x61, 0x7a,
                0xfd, 0xc9, 0x54, 0x86, 0xa2, 0xbf, 0xe1, 0x65, 0xf5, 0x16, 0x3a, 0x36, 0xed, 0x99,
                0x8f, 0x0d, 0x92, 0x35,
            ],
        ),
        (
            119,
            [
                0x75, 0xd1, 0xdd, 0xb2, 0xac, 0xf3, 0x5c, 0x4b, 0x59, 0x13, 0x27, 0x77, 0x01, 0xea,
                0x70, 0xe0, 0xa9, 0x6c, 0xbf, 0x09, 0x87, 0xaf, 0x0f, 0x3a, 0x7d, 0xab, 0xe1, 0xaf,
                0xc4, 0xd6, 0x71, 0xb9,
            ],
        ),
        (
            120,
            [
                0x06, 0x15, 0x7d, 0x8e, 0x43, 0xe6, 0x3a, 0x5d, 0x03, 0xd4, 0xe4, 0xca, 0x99, 0xb3,
                0xcd, 0x4c, 0xab, 0x39, 0x11, 0xc1, 0x1f, 0x1c, 0x5b, 0x17, 0x42, 0x4e, 0xe3, 0x6c,
                0x67, 0x66, 0x2b, 0x5a,
            ],
        ),
        (
            121,
            [
                0x40, 0x86, 0x5e, 0x36, 0x5e, 0xf8, 0x6d, 0xc5, 0xbf, 0xc5, 0x54, 0x03, 0x8e, 0xbb,
                0x2c, 0x77, 0xb7, 0xf0, 0xa6, 0x18, 0xa1, 0xbe, 0xc0, 0x08, 0x30, 0xe7, 0x42, 0xea,
                0x35, 0x83, 0x7e, 0x16,
            ],
        ),
        (
            200,
            [
                0x6b, 0x28, 0xb9, 0x42, 0x2b, 0xe6, 0x5d, 0x71, 0x6e, 0x76, 0x76, 0x6a, 0xb7, 0x4d,
                0x14, 0xe6, 0x30, 0x99, 0x58, 0x66, 0xd0, 0x9f, 0xdf, 0xc1, 0x77, 0x18, 0x4a, 0xe4,
                0x51, 0xc1, 0xa2, 0x1f,
            ],
        ),
        (
            1000,
            [
                0x95, 0x31, 0x1d, 0x7b, 0x41, 0x68, 0x38, 0x1d, 0x43, 0x9c, 0x3c, 0xed, 0xfa, 0xc6,
                0x15, 0x3a, 0x1e, 0x81, 0x35, 0x33, 0x13, 0x6f, 0xcd, 0x1d, 0x95, 0x5e, 0x32, 0x5a,
                0xdc, 0xd0, 0xbd, 0x53,
            ],
        ),
        (
            100_000,
            [
                0x8d, 0x0f, 0xa4, 0x3b, 0xf1, 0x48, 0xdb, 0xad, 0xd6, 0xe0, 0x9d, 0xbc, 0xbb, 0xf2,
                0x75, 0x7a, 0x30, 0xfe, 0xfb, 0x55, 0x98, 0xd3, 0x12, 0xef, 0xb1, 0xa0, 0x44, 0xf1,
                0xf9, 0xe5, 0x96, 0xd1,
            ],
        ),
    ];

    #[test]
    fn test_against_independent_oracle_boundary_lengths() {
        for (len, expected) in VECTORS {
            let mut rng = synthvid_scene::Rng::from_seed(synthvid_scene::Seed::new(0x1234_5678));
            let mut input = vec![0u8; len];
            for byte in &mut input {
                *byte = u8::try_from(rng.next_bounded(256)).unwrap();
            }

            let result = sha256(&input);
            assert_eq!(
                result.as_bytes(),
                &expected,
                "mismatch for input length {len}"
            );
        }
    }

    #[test]
    fn test_streaming_vs_oneshot() {
        // Test that streaming with awkward chunk sizes produces the same result as one-shot.
        let seed = [
            0x12u8, 0x34u8, 0x56u8, 0x78u8, 0x9au8, 0xbcu8, 0xdeu8, 0xf0u8,
        ];
        let mut input = vec![0u8; 300];
        for (i, byte) in input.iter_mut().enumerate() {
            *byte = seed[i % seed.len()];
        }
        let oneshot_result = sha256(&input);

        // Hash with streaming in awkward chunk sizes: 1, 63, 64, 65, 127
        let mut streaming = Sha256::new();
        let mut offset = 0;

        // Feed 1 byte
        streaming.update(&input[offset..=offset]);
        offset += 1;
        // Feed 63 bytes (would complete a 64-byte block with previous 1)
        streaming.update(&input[offset..offset + 63]);
        offset += 63;
        // Feed 64 bytes (exactly one block)
        streaming.update(&input[offset..offset + 64]);
        offset += 64;
        // Feed 65 bytes (one block plus 1)
        streaming.update(&input[offset..offset + 65]);
        offset += 65;
        // Feed remaining bytes in 127-byte chunks
        while offset < input.len() {
            let chunk_size = 127.min(input.len() - offset);
            streaming.update(&input[offset..offset + chunk_size]);
            offset += chunk_size;
        }

        let streaming_result = streaming.finalize();
        assert_eq!(
            streaming_result.as_bytes(),
            oneshot_result.as_bytes(),
            "streaming interface produced different result than one-shot"
        );
    }
}

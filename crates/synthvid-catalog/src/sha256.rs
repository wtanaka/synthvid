//! SHA-256 hash function.
//!
//! Implemented from FIPS 180-4 "Secure Hash Standard (SHS)".

use core::fmt;
use core::ops::{BitAnd, BitXor, Not};

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

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Digest")
            .field(&format_args!("{self}"))
            .finish()
    }
}

/// SHA-256 hash state.
#[derive(Clone, Copy, Debug)]
struct State([Word; 8]);

impl State {
    /// Initialize a new SHA-256 state with the standard initial hash values
    /// (h0 through h7, in order).
    const fn new() -> Self {
        Self([
            Word(0x6a09_e667_u32),
            Word(0xbb67_ae85_u32),
            Word(0x3c6e_f372_u32),
            Word(0xa54f_f53a_u32),
            Word(0x510e_527f_u32),
            Word(0x9b05_688c_u32),
            Word(0x1f83_d9ab_u32),
            Word(0x5be0_cd19_u32),
        ])
    }
}

/// One 512-bit message block, the unit the compression function consumes.
#[derive(Clone, Copy, Debug)]
struct Block([u8; 64]);

/// How many whole blocks a hasher has compressed, modulo 2^64.
///
/// Counting blocks rather than bytes keeps the message length exact without
/// converting a `usize`: the length in bits is this count shifted left by
/// nine, plus the bits still pending.
#[derive(Clone, Copy, Debug)]
struct BlockCount(u64);

impl BlockCount {
    /// The count after one more block, modulo 2^64.
    ///
    /// `a + 1` is `2 * midpoint(a, 1)` plus the low bit of `a ^ 1`;
    /// `midpoint` cannot overflow, and the shift drops exactly the carry out
    /// of bit 63.
    const fn incremented(self) -> Self {
        Self((self.0.midpoint(1) << 1) | ((self.0 ^ 1) & 1))
    }
}

/// The bytes of a partial block, fewer than 64 between calls.
#[derive(Clone, Debug)]
struct Pending(Vec<u8>);

/// A streaming SHA-256 hasher.
///
/// Allows incremental hashing of data as it becomes available, avoiding the
/// need to buffer entire inputs in memory.
#[derive(Clone, Debug)]
pub struct Sha256 {
    /// Current hash state.
    state: State,
    /// The bytes of a partial block, fewer than 64.
    pending: Pending,
    /// Whole blocks compressed so far.
    blocks: BlockCount,
}

impl Sha256 {
    /// Create a new SHA-256 hasher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: State::new(),
            pending: Pending(Vec::with_capacity(64)),
            blocks: BlockCount(0),
        }
    }

    /// Compresses one block into the state and counts it.
    fn compress(&mut self, block: &Block) {
        process_block(block, &mut self.state);
        self.blocks = self.blocks.incremented();
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
    pub fn update<B>(&mut self, bytes: &B)
    where
        B: AsRef<[u8]> + ?Sized,
    {
        let mut rest = bytes.as_ref().iter();
        if !self.pending.0.is_empty() {
            while self.pending.0.len() < 64 {
                match rest.next() {
                    Some(byte) => self.pending.0.push(*byte),
                    None => return,
                }
            }
            let (full, _) = self.pending.0.as_chunks::<64>();
            let first_block = full.first().copied().map(Block);
            self.pending.0.clear();
            if let Some(block) = first_block {
                self.compress(&block);
            }
        }
        let (chunks, remainder) = rest.as_slice().as_chunks::<64>();
        for chunk in chunks {
            self.compress(&Block(*chunk));
        }
        self.pending.0.extend_from_slice(remainder);
    }

    /// Finalize the hasher and return the digest.
    #[must_use]
    pub fn finalize(mut self) -> Digest {
        // Fewer than 64 bytes are pending, so the low byte of the length is
        // the whole length, and shifted left by three it fits below bit 9,
        // where the block count shifted left by nine begins. The two
        // therefore combine with `|` exactly as they would add.
        let [pending_len, ..] = self.pending.0.len().to_le_bytes();
        let bit_len = (self.blocks.0 << 9_i32) | (u64::from(pending_len) << 3_i32);
        let mut last = Block([0_u8; 64]);
        for (dst, src) in last.0.iter_mut().zip(&self.pending.0) {
            *dst = *src;
        }
        if let Some(marker) = last.0.get_mut(self.pending.0.len()) {
            *marker = 0x80;
        }
        if self.pending.0.len() >= 56 {
            process_block(&last, &mut self.state);
            last = Block([0_u8; 64]);
        }
        if let Some(dst) = last.0.get_mut(56..64) {
            dst.copy_from_slice(&bit_len.to_be_bytes());
        }
        process_block(&last, &mut self.state);
        let mut result = [0_u8; 32];
        let (output_chunks, _) = result.as_chunks_mut::<4>();
        for (chunk, word) in output_chunks.iter_mut().zip(self.state.0) {
            chunk.copy_from_slice(&word.0.to_be_bytes());
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
pub fn sha256<B>(input: &B) -> Digest
where
    B: AsRef<[u8]> + ?Sized,
{
    let mut hasher = Sha256::new();
    hasher.update(input);
    hasher.finalize()
}

/// Wrapper type for SHA-256 words, which add modulo 2^32.
///
/// `+` is deliberately not implemented: `clippy::arithmetic_side_effects`
/// flags any use of the `+` operator, including through a custom `Add`
/// impl, so addition modulo 2^32 is exposed as the [`Word::plus`] method.
#[derive(Clone, Copy, Debug)]
struct Word(u32);

impl BitXor for Word {
    type Output = Self;

    fn bitxor(self, other: Self) -> Self {
        Self(self.0 ^ other.0)
    }
}

impl BitAnd for Word {
    type Output = Self;

    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

impl Not for Word {
    type Output = Self;

    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl Word {
    /// Rotate right by `N` bits.
    const fn rotate_right<const N: u32>(self) -> Self {
        Self(self.0.rotate_right(N))
    }

    /// Add modulo 2^32, as FIPS 180-4 defines word addition.
    ///
    /// `a + b` is `2 * midpoint(a, b)` plus the low bit of `a ^ b`:
    /// `midpoint` rounds down and cannot overflow, so the sum's parity is
    /// all it loses, and the shift drops exactly the carry out of bit 31.
    const fn plus(self, other: Self) -> Self {
        Self((self.0.midpoint(other.0) << 1) | ((self.0 ^ other.0) & 1))
    }
}

/// Round constants (first 32 bits of the fractional parts of the cube
/// roots of the first 64 primes), per FIPS 180-4.
static ROUND_CONSTANTS: [Word; 64] = [
    Word(0x428a_2f98),
    Word(0x7137_4491),
    Word(0xb5c0_fbcf),
    Word(0xe9b5_dba5),
    Word(0x3956_c25b),
    Word(0x59f1_11f1),
    Word(0x923f_82a4),
    Word(0xab1c_5ed5),
    Word(0xd807_aa98),
    Word(0x1283_5b01),
    Word(0x2431_85be),
    Word(0x550c_7dc3),
    Word(0x72be_5d74),
    Word(0x80de_b1fe),
    Word(0x9bdc_06a7),
    Word(0xc19b_f174),
    Word(0xe49b_69c1),
    Word(0xefbe_4786),
    Word(0x0fc1_9dc6),
    Word(0x240c_a1cc),
    Word(0x2de9_2c6f),
    Word(0x4a74_84aa),
    Word(0x5cb0_a9dc),
    Word(0x76f9_88da),
    Word(0x983e_5152),
    Word(0xa831_c66d),
    Word(0xb003_27c8),
    Word(0xbf59_7fc7),
    Word(0xc6e0_0bf3),
    Word(0xd5a7_9147),
    Word(0x06ca_6351),
    Word(0x1429_2967),
    Word(0x27b7_0a85),
    Word(0x2e1b_2138),
    Word(0x4d2c_6dfc),
    Word(0x5338_0d13),
    Word(0x650a_7354),
    Word(0x766a_0abb),
    Word(0x81c2_c92e),
    Word(0x9272_2c85),
    Word(0xa2bf_e8a1),
    Word(0xa81a_664b),
    Word(0xc24b_8b70),
    Word(0xc76c_51a3),
    Word(0xd192_e819),
    Word(0xd699_0624),
    Word(0xf40e_3585),
    Word(0x106a_a070),
    Word(0x19a4_c116),
    Word(0x1e37_6c08),
    Word(0x2748_774c),
    Word(0x34b0_bcb5),
    Word(0x391c_0cb3),
    Word(0x4ed8_aa4a),
    Word(0x5b9c_ca4f),
    Word(0x682e_6ff3),
    Word(0x748f_82ee),
    Word(0x78a5_636f),
    Word(0x84c8_7814),
    Word(0x8cc7_0208),
    Word(0x90be_fffa),
    Word(0xa450_6ceb),
    Word(0xbef9_a3f7),
    Word(0xc671_78f2),
];

/// The eight FIPS 180-4 working variables for one block's compression.
///
/// Field `alpha` is the FIPS variable `a`, and so on through `hotel`.
struct Working {
    /// First working variable (a).
    alpha: Word,
    /// Second working variable (b).
    bravo: Word,
    /// Third working variable (c).
    charlie: Word,
    /// Fourth working variable (d).
    delta: Word,
    /// Fifth working variable (e).
    echo: Word,
    /// Sixth working variable (f).
    foxtrot: Word,
    /// Seventh working variable (g).
    golf: Word,
    /// Eighth working variable (h).
    hotel: Word,
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
fn process_block(block: &Block, state: &mut State) {
    // The rolling window starts as the first 16 message-schedule words,
    // read directly from the block.
    let mut window = [Word(0); 16];
    let (chunks, _) = block.0.as_chunks::<4>();
    for (slot, chunk) in window.iter_mut().zip(chunks) {
        *slot = Word(u32::from_be_bytes(*chunk));
    }

    // Initialize working variables
    let mut working = Working {
        alpha: state.0[0],
        bravo: state.0[1],
        charlie: state.0[2],
        delta: state.0[3],
        echo: state.0[4],
        foxtrot: state.0[5],
        golf: state.0[6],
        hotel: state.0[7],
    };

    // One round of the compression function, given this round's schedule
    // word and round constant.
    let mut round = |constant: Word, w_i: Word| {
        let sigma1 = working.echo.rotate_right::<6>()
            ^ working.echo.rotate_right::<11>()
            ^ working.echo.rotate_right::<25>();
        let ch = (working.echo & working.foxtrot) ^ ((!working.echo) & working.golf);
        let temp1 = working.hotel.plus(sigma1).plus(ch).plus(constant).plus(w_i);
        let sigma0 = working.alpha.rotate_right::<2>()
            ^ working.alpha.rotate_right::<13>()
            ^ working.alpha.rotate_right::<22>();
        let maj = (working.alpha & working.bravo)
            ^ (working.alpha & working.charlie)
            ^ (working.bravo & working.charlie);
        let temp2 = sigma0.plus(maj);

        working.hotel = working.golf;
        working.golf = working.foxtrot;
        working.foxtrot = working.echo;
        working.echo = working.delta.plus(temp1);
        working.delta = working.charlie;
        working.charlie = working.bravo;
        working.bravo = working.alpha;
        working.alpha = temp1.plus(temp2);
    };

    // Rounds 0..16 consume the schedule words as loaded from the block.
    for (&constant, &w_i) in ROUND_CONSTANTS[..16].iter().zip(window.iter()) {
        round(constant, w_i);
    }

    // Rounds 16..64 expand one new schedule word per round from the
    // rolling window before consuming it; see the doc comment above for
    // why the backreferences become fixed offsets 0, 1, 9, and 14.
    for &constant in &ROUND_CONSTANTS[16..64] {
        let s0 =
            window[1].rotate_right::<7>() ^ window[1].rotate_right::<18>() ^ Word(window[1].0 >> 3);
        let s1 = window[14].rotate_right::<17>()
            ^ window[14].rotate_right::<19>()
            ^ Word(window[14].0 >> 10);
        let w_i = window[0].plus(s0).plus(window[9]).plus(s1);
        window.rotate_left(1);
        window[15] = w_i;

        round(constant, w_i);
    }

    // Add compressed chunk to current hash value
    state.0[0] = state.0[0].plus(working.alpha);
    state.0[1] = state.0[1].plus(working.bravo);
    state.0[2] = state.0[2].plus(working.charlie);
    state.0[3] = state.0[3].plus(working.delta);
    state.0[4] = state.0[4].plus(working.echo);
    state.0[5] = state.0[5].plus(working.foxtrot);
    state.0[6] = state.0[6].plus(working.golf);
    state.0[7] = state.0[7].plus(working.hotel);
}

#[cfg(test)]
mod tests;

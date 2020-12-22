// Public-domain implementation from:
//    https://github.com/teor2345/privcount_shamir/

// Implements a prime field modulo some prime of form 2^A - 2^B - 1.
//
// Tries to be fairly efficient, and to not have timing side-channels.
//
// Certain constraints are placed on A and B, see below.

use num::traits::{Num, One, Zero};
use serde::{Deserialize, Serialize};
use std::cmp::{Eq, PartialEq};
use std::convert::From;
use std::fmt::{self, Display, Formatter, LowerHex, UpperHex};
use std::hash::{Hash, Hasher};
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};
use std::ops::{AddAssign, DivAssign, MulAssign, RemAssign, SubAssign};

// Here are the constants that determine our prime:
//
// number of bits in our field elements
const N_BITS: u64 = 62;
// Which bit (other than bit 0) do we clear in our prime?
const OFFSET_BIT: u64 = 30;
// order of the prime field
const PRIME_ORDER: u64 = (1 << N_BITS) - (1 << OFFSET_BIT) - 1;

// There are some constraints on those constants, as described here:
//
// 2^N_BITS - (2^OFFSET_BIT + 1) must be prime; we do all of our
//   arithmetic modulo this prime.
// Choose OFFSET_BIT low, and less than N_BITS/2.
// Our recip() implementation requires OFFSET_BIT != 2.
// Choose N_BITS even, and no more than 64 - 2, and no less than 34.

// READ THIS TO UNDERSTAND:
//
//  We represent values mod P in four different u64-based forms.
//  For every form, the u64 value "v" represents the field element "v % P".
//
//  0. Unreduced:  v can be any u64.
//  1. Bit-reduced once: v is in range 0..FE_VAL_MAX
//  2. Bit-reduced twice: v is in range 0..FULL_BITS_MASK
//  3. Fully reduced: v is in range 0..PRIME_ORDER - 1.
//
//  The function bit_reduce_once() converts from [0] to [1] and from
//  [1] to [2].  The function reduce_by_p() converts from [2] to [3].
//
//  When we store a value internally in an FE object, we use format [1].
//  When we expose a value to the caller, or we compare two FEs for equality,
//  we use format [3].
//
//  We accept format [0] for input.
//
//  We use formats [0] and [1] for intermediate calculations.

// Mask to mask off all bits that aren't used in the field elements.
const FULL_BITS_MASK: u64 = (1 << N_BITS) - 1;

// We use these macros to check invariants.

// Number of bits in a u64 which we don't use.
const REMAINING_BITS: u64 = 64 - N_BITS;
// Largest remaining value after we take a u64 and shift away the
// bits that we want to use in our field.
const MAX_EXCESS: u64 = (1 << REMAINING_BITS) - 1;
// Largest value to use in our field elements.  This will spill
// over our regular bit mask by a little, since we don't store stuff
// in a fully bit-reduced form.
const FE_VAL_MAX: u64 = FULL_BITS_MASK + (MAX_EXCESS << OFFSET_BIT) + MAX_EXCESS;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct FE {
    // This value is stored in a bit-reduced form: it will be in range
    // 0..FE_VAL_MAX.  It is equivalent modulo PRIME_ORDER to the
    // actual value of this field element
    val: u64,
}

// Given a value in range 0..U64_MAX, returns a value in range 0..FE_VAL_MAX.
//
// (Given a value in range 0..FE_VAL_MAX, the output is in range
// 0..FULL_BITS_MASK.)
fn bit_reduce_once(v: u64) -> u64 {
    // Excess is in range 0..MAX_EXCESS
    let excess = v >> N_BITS;
    // Lowpart is in range 0..FULL_BITS_MASK
    let lowpart = v & FULL_BITS_MASK;
    // Result is at most FE_VAL_MAX
    let result = lowpart + excess + (excess << OFFSET_BIT);
    debug_assert!(result <= FE_VAL_MAX);
    result
}

// Returns "if v > PRIME_ORDER { v - PRIME_ORDER } else { v }".
//
// We only call this when it will produce a value in range 0..PRIME_ORDER-1.
fn reduce_by_p(v: u64) -> u64 {
    debug_assert!(v < PRIME_ORDER * 2);
    let difference = v.wrapping_sub(PRIME_ORDER);
    let overflow_bit = difference & (1 << 63);
    let mask = ((overflow_bit as i64) >> 63) as u64;

    (mask & v) | ((!mask) & difference)
}

impl FE {
    // Construct a new FE value.  Accepts any u64, and creates an FE
    // that represents that value modulo PRIME_ORDER.
    pub fn new(v: u64) -> Self {
        // This bit_reduce_once ensures that the value is in range
        // 0..FE_VAL_MAX.
        FE {
            val: bit_reduce_once(v),
        }
    }
    // Construct a new FE value from a u64 value, such that if the
    // inputs to this function are uniform random u64s, then all of the
    // non-None outputs of this function are uniform random FEs.
    //
    // The implementation should try to return a non-None value for
    // the majority of inputs.
    pub fn from_u64_unbiased(v: u64) -> Option<Self> {
        // We first mask out the high bits of v, and then return a value
        // only when the masked value is less than PRIME_ORDER.  This
        // will be the case with probability = PRIME_ORDER / (1<<N_BITS),
        // = 1 - 2^-32 - 1^-62.
        FE::from_reduced(v & FULL_BITS_MASK)
    }
    // Construct a new FE value if v is in range 0..PRIME_ORDER-1.
    // If it is not, return None.
    pub fn from_reduced(v: u64) -> Option<Self> {
        if v < PRIME_ORDER {
            Some(FE { val: v })
        } else {
            None
        }
    }
    fn new_raw(v: u32) -> Self {
        // Since v <= u32::MAX, we know that it is less than FE_VAL_MAX.
        debug_assert!((std::u32::MAX as u64) < FE_VAL_MAX);
        FE { val: v as u64 }
    }
    // Return the value of this FE, as an integer in range 0..PRIME_ORDER-1.
    pub fn value(self) -> u64 {
        // self.val is already bit-reduced once, so we only have to
        // bit-reduce it once more to put it in range 0..FULL_BITS_MASK.
        // Then, reduce_by_p will put it in range 0..PRIME_ORDER - 1
        reduce_by_p(bit_reduce_once(self.val))
    }
    // Compute the reciprocal of this value.
    pub fn recip(self) -> Self {
        debug_assert_ne!(self, FE::new_raw(0));

        // To compute the reciprical, we need to compute
        // self^E where E = (PRIME_ORDER-2).
        //
        // Since OFFSET_BIT != 2, E has every bit in (0..N_BITS-1)
        // set, except for bits 1 and OFFSET_BIT.  In other words,
        // it looks like 0b11111111..11101111..01

        // Simple version of exponention-by-squaring algorithm.
        let mut x = self;
        let mut y = FE::new(1);

        // Bit 0 is set.
        y = x * y;
        x = x * x;
        // Bit 1 is clear.
        x = x * x;
        // Bits 2 through offset_bit-1 are set.
        for _ in 2..(OFFSET_BIT) {
            y = x * y;
            x = x * x;
        }
        // OFFSET_BIT is clear
        x = x * x;
        // OFFSET_BIT + 1 through N_BITS-2
        for _ in (OFFSET_BIT + 1)..(N_BITS - 1) {
            y = x * y;
            x = x * x;
        }
        x * y
    }
}

// From implementations: these values are always in-range.
impl From<u8> for FE {
    fn from(v: u8) -> FE {
        FE::new_raw(v as u32)
    }
}
impl From<u16> for FE {
    fn from(v: u16) -> FE {
        FE::new_raw(v as u32)
    }
}
impl From<u32> for FE {
    fn from(v: u32) -> FE {
        FE::new_raw(v as u32)
    }
}

impl From<FE> for u64 {
    fn from(v: FE) -> u64 {
        v.value()
    }
}
impl Zero for FE {
    fn zero() -> FE {
        FE::new_raw(0)
    }
    fn is_zero(&self) -> bool {
        self.value() == 0
    }
}
impl One for FE {
    fn one() -> FE {
        FE::new_raw(1)
    }
}

impl Add for FE {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        // This sum stay in range, since FE_MAX_VAL * 2 < U64_MAX.
        // The FE::new call will bit-reduce the result.
        FE::new(self.val + rhs.val)
    }
}

impl Neg for FE {
    type Output = Self;
    fn neg(self) -> Self {
        // PRIME_ORDER * 2 is less than u64::MAX, since N_BITS <= 62.
        // FE::new call will bit-reduce the result.
        FE::new(PRIME_ORDER * 2 - self.val)
    }
}

impl Sub for FE {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self + (-rhs)
    }
}

impl PartialEq for FE {
    fn eq(&self, rhs: &Self) -> bool {
        self.value() == rhs.value()
    }
}
impl Eq for FE {}

impl Hash for FE {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        hasher.write_u64(self.value())
    }
}

impl AddAssign for FE {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}
impl SubAssign for FE {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl Display for FE {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        Display::fmt(&self.value(), f)
    }
}

impl UpperHex for FE {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        UpperHex::fmt(&self.value(), f)
    }
}

impl LowerHex for FE {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        LowerHex::fmt(&self.value(), f)
    }
}

impl Default for FE {
    fn default() -> Self {
        FE::new_raw(0)
    }
}

impl Mul for FE {
    type Output = Self;

    // Implement multiplication. We have separate implementations
    // depending on whether we have u128 support or not.

    fn mul(self, rhs: Self) -> Self {
        // If we have u128, we are much happier.

        // Here's our bit-reduction algorithm once again, this time
        // taking a u128 as input.
        fn bit_reduce_once_128(v: u128) -> u128 {
            let low = v & (FULL_BITS_MASK as u128);
            let high = v >> N_BITS;
            low + (high << OFFSET_BIT) + high
        }

        // This product is is most FE_VAL_MAX^2; FE_VAL_MAX is less
        // than 2^63, so this value is less than 2^126.  No overflow
        // here!
        let product = (self.val as u128) * (rhs.val as u128);

        // The first two bit-reduces are sufficient to make the produce
        // less than 2^64.  Once we've done that, FE::new can accept it
        // (and do another bit-reduction).
        let result = bit_reduce_once_128(bit_reduce_once_128(product));
        debug_assert!(result < (1 << 64));
        FE::new(result as u64)
    }
}

impl Div for FE {
    type Output = Self;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self {
        self * rhs.recip()
    }
}

impl Rem for FE {
    type Output = Self;
    // not sure why you would want this.... XXXX
    // .... but it makes the Num trait work out.
    fn rem(self, rhs: Self) -> Self {
        self - (self / rhs)
    }
}

impl MulAssign for FE {
    fn mul_assign(&mut self, other: Self) {
        *self = *self * other;
    }
}
impl DivAssign for FE {
    fn div_assign(&mut self, other: Self) {
        *self = *self / other;
    }
}
impl RemAssign for FE {
    fn rem_assign(&mut self, other: Self) {
        *self = *self % other;
    }
}

impl<'a> Add<&'a FE> for FE {
    type Output = Self;
    fn add(self, rhs: &Self) -> FE {
        self + *rhs
    }
}
impl<'a> Sub<&'a FE> for FE {
    type Output = Self;
    fn sub(self, rhs: &Self) -> FE {
        self - *rhs
    }
}

impl<'a, 'b> Sub<&'b FE> for &'a FE {
    type Output = FE;

    fn sub(self, rhs: &'b FE) -> FE {
        *self - *rhs
    }
}

impl<'a> Mul<&'a FE> for FE {
    type Output = Self;
    fn mul(self, rhs: &Self) -> FE {
        self * *rhs
    }
}
impl<'a> Div<&'a FE> for FE {
    type Output = Self;
    fn div(self, rhs: &Self) -> FE {
        self / *rhs
    }
}
impl<'a> Rem<&'a FE> for FE {
    type Output = Self;
    fn rem(self, rhs: &Self) -> FE {
        self % *rhs
    }
}

impl Num for FE {
    type FromStrRadixErr = &'static str;
    fn from_str_radix(s: &str, radix: u32) -> Result<Self, &'static str> {
        let u = u64::from_str_radix(s, radix).map_err(|_| "Bad num")?;
        FE::from_reduced(u).ok_or("Too big")
    }
}

#[cfg(test)]
mod tests {
    //use math::*;
    use super::*;

    fn maxrep() -> FE {
        FE { val: FE_VAL_MAX }
    }
    fn fullbits() -> FE {
        FE {
            val: FULL_BITS_MASK,
        }
    }

    #[test]
    fn constants_in_range() {
        assert!(N_BITS % 2 == 0);
        assert!(N_BITS <= 62);
        assert!(OFFSET_BIT < N_BITS / 2);
        assert!(OFFSET_BIT != 2);
    }
    #[test]
    fn prime_is_prime() {
        use primal;
        assert!(primal::is_prime(PRIME_ORDER));
    }
    #[test]
    fn test_values() {
        assert_eq!(FE::new(0).value(), 0);
        assert_eq!(FE::new(1337).value(), 1337);
        assert_eq!(FE::new(PRIME_ORDER).value(), 0);
        assert_eq!(FE::new(PRIME_ORDER + 1).value(), 1);
        assert_eq!(FE::new(PRIME_ORDER - 1).value(), PRIME_ORDER - 1);
        assert_eq!(FE::new(PRIME_ORDER).value(), 0);
        assert_eq!(FE::new(PRIME_ORDER * 2).value(), 0);
        assert_eq!(FE::new(!0u64).value(), (!0u64) % PRIME_ORDER);
        assert_eq!(maxrep().value(), FE_VAL_MAX - PRIME_ORDER);
    }
    #[test]
    fn test_equivalence() {
        assert_eq!(FE::new(0), FE::new(PRIME_ORDER));
        assert_eq!(FE::new(1), FE::new(PRIME_ORDER + 1));
        assert_eq!(FE::new(1), FE::new(PRIME_ORDER * 2 + 1));
        assert_eq!(FE::new(PRIME_ORDER - 50), FE::new(PRIME_ORDER * 4 - 50));
        assert_eq!(maxrep(), FE::new(FE_VAL_MAX - PRIME_ORDER));
    }
    #[test]
    fn test_add_sub() {
        assert_eq!(FE::new(0) - FE::new(100), FE::new(PRIME_ORDER - 100));
        assert_eq!(FE::new(100) - FE::new(5), FE::new(95));
        assert_eq!(FE::new(100) - FE::new(105), FE::new(PRIME_ORDER - 5));
        assert_eq!(FE::new(300) - FE::new(PRIME_ORDER + 1), FE::new(299));
        assert_eq!(FE::new(1050) + FE::new(1337), FE::new(2387));
        assert_eq!(FE::new(1337) + FE::new(PRIME_ORDER - 37), FE::new(1300));
        assert_eq!(-FE::new(10) + (-FE::new(15)), -FE::new(25));

        assert_eq!(-maxrep(), FE::new(PRIME_ORDER * 2 - FE_VAL_MAX));
        assert_eq!(maxrep() + maxrep(), FE::new((FE_VAL_MAX - PRIME_ORDER) * 2));
        assert_eq!(maxrep() - maxrep(), FE::zero());
        assert_eq!(FE::zero() - maxrep(), -maxrep());

        assert_eq!(
            FE::new(1000) - maxrep(),
            FE::new(PRIME_ORDER * 2 - FE_VAL_MAX + 1000)
        );

        assert_eq!(-fullbits(), FE::new(PRIME_ORDER * 2 - FULL_BITS_MASK));
        assert_eq!(FE::zero() - fullbits(), -fullbits());
    }
    #[test]
    fn mult() {
        assert_eq!(FE::new(0) * FE::new(1000), FE::new(0));
        assert_eq!(FE::new(999) * FE::new(1000), FE::new(999000));
        assert_eq!(FE::new(PRIME_ORDER) * FE::new(PRIME_ORDER), FE::new(0));
        assert_eq!(
            FE::new(PRIME_ORDER - 1) * FE::new(PRIME_ORDER - 1),
            FE::new(1)
        );
        assert_eq!(
            FE::new(PRIME_ORDER - 2) * FE::new(PRIME_ORDER - 2),
            FE::new(4)
        );

        assert_eq!(
            maxrep() * maxrep(),
            FE::new(FE_VAL_MAX % PRIME_ORDER) * FE::new(FE_VAL_MAX % PRIME_ORDER)
        );
        assert_eq!(
            fullbits() * fullbits(),
            FE::new(FULL_BITS_MASK % PRIME_ORDER) * FE::new(FULL_BITS_MASK % PRIME_ORDER)
        )
    }
    #[test]
    fn recip() {
        assert_eq!(FE::new(1).recip(), FE::new(1));
        assert_eq!(FE::new(999).recip() * FE::new(999), FE::new(1));
        assert_eq!(FE::new(999).recip(), FE::new(2885188949795824624));
        assert_eq!(FE::new(999), FE::new(2885188949795824624).recip());
    }
    #[test]
    fn construct_maybe() {
        assert_eq!(FE::from_reduced(12345), Some(FE::new(12345)));
        assert_eq!(
            FE::from_reduced(PRIME_ORDER - 1),
            Some(FE::new(PRIME_ORDER - 1))
        );
        assert_eq!(FE::from_reduced(PRIME_ORDER), None);
        assert_eq!(FE::from_reduced(PRIME_ORDER * 2), None);

        assert_eq!(FE::from_u64_unbiased(12345), Some(FE::new(12345)));
        let hibit = 1 << N_BITS;
        assert_eq!(FE::from_u64_unbiased(12345 + hibit), Some(FE::new(12345)));
        assert_eq!(
            FE::from_u64_unbiased(PRIME_ORDER - 1),
            Some(FE::new(PRIME_ORDER - 1))
        );
        assert_eq!(FE::from_u64_unbiased(PRIME_ORDER), None);
        assert_eq!(
            FE::from_u64_unbiased(PRIME_ORDER - 1 + hibit),
            Some(FE::new(PRIME_ORDER - 1))
        );
        assert_eq!(
            FE::from_u64_unbiased(PRIME_ORDER - 1 + hibit * 2),
            Some(FE::new(PRIME_ORDER - 1))
        );
        assert_eq!(FE::from_u64_unbiased(PRIME_ORDER + hibit), None);
        assert_eq!(FE::from_u64_unbiased(PRIME_ORDER + hibit * 2), None);
    }

    /*
    fn mul_slow(a: FE, b: FE) -> FE {
        use num::bigint::BigUint;
        use num::traits::cast::FromPrimitive;
        use num::traits::cast::ToPrimitive;
        let a_big = BigUint::from_u64(a.val).unwrap();
        let b_big = BigUint::from_u64(b.val).unwrap();
        let product = (a_big * b_big) % PRIME_ORDER;
        FE::new(product.to_u64().unwrap())
    }
    */

    /*
    use quickcheck::{Arbitrary, Gen};
    impl Arbitrary for FE {
        fn arbitrary<G: Gen>(g: &mut G) -> FE {
            loop {
                let v  = FE::from_u64_unbiased(g.next_u64());
                match v {
                    Some(x) => return x,
                    None => continue,
                }
            }
        }
    }
    quickcheck! {
        fn p_multiply(a : FE, b : FE) -> bool {
            // println!("{:?} * {:?}", a, b);
            a * b == mul_slow(a,b)
        }

        fn p_recip(a : FE) -> bool {
            // println!("1 / {:?}", a);
            a * a.recip() == FE::new(1)
        }

        fn p_div(a : FE, b : FE) -> bool {
            (a / b) * b == a
        }
    }
    */
}

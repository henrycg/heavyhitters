use crate::fastfield::FE;
#[cfg(test)]
use crate::Share;

use num_bigint::{BigUint, RandBigInt};
use serde::Deserialize;
use serde::Serialize;
use std::cmp::Ordering;
use std::convert::TryInto;
use std::u32;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldElm {
    value: BigUint,
}

// 255-bit modulus:   p = 2^255 - 10
const MODULUS_STR: &[u8] = b"7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed";

// 127-bit modulus:   p = 2^127 - 1
//const MODULUS_STR: &[u8] = b"7fffffffffffffffffffffffffffffff";

//  63-bit modulus:   p = 2^63 - 25;
const MODULUS_64: u64 = 9223372036854775783u64;
const MODULUS_64_BIG: u128 = 9223372036854775783u128;

lazy_static! {
    static ref MODULUS: FieldElm =
        FieldElm::from_hex(MODULUS_STR).expect("Could not parse modulus");
    static ref MODULUS_DUMMY: Dummy = Dummy::from(7);
}

impl FieldElm {
    pub fn from_hex(inp: &[u8]) -> Option<FieldElm> {
        BigUint::parse_bytes(inp, 16).map(|value| FieldElm { value })
    }

    pub fn to_vec(&self, len: usize) -> Vec<FieldElm> {
        std::iter::repeat(self.clone()).take(len).collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Dummy {
    value: u32,
}

/*******/
impl From<u32> for Dummy {
    fn from(_inp: u32) -> Self {
        Dummy { value: 0 }
    }
}

impl From<BigUint> for Dummy {
    fn from(_inp: BigUint) -> Self {
        Dummy { value: 0 }
    }
}

impl Ord for Dummy {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for Dummy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.value.cmp(&other.value))
    }
}

impl crate::Group for Dummy {
    fn zero() -> Self {
        Dummy::from(0)
    }

    fn one() -> Self {
        Dummy::from(1)
    }

    fn add(&mut self, other: &Self) {
        //*self = FieldElm::from((&self.value + &other.value) % &MODULUS.value);
        self.value += &other.value;
        self.value %= &MODULUS.value;
    }

    fn mul(&mut self, other: &Self) {
        self.value *= &other.value;
        self.value %= &MODULUS.value;
    }

    fn add_lazy(&mut self, other: &Self) {
        self.value += &other.value;
    }

    fn mul_lazy(&mut self, other: &Self) {
        self.value *= &other.value;
    }

    fn reduce(&mut self) {
        self.value %= &MODULUS.value;
    }

    fn sub(&mut self, other: &Self) {
        // XXX not constant time
        if self.value < other.value {
            self.value += &MODULUS_DUMMY.value;
        }

        *self = Dummy::from(self.value - other.value);
    }

    fn negate(&mut self) {
        self.value = MODULUS_DUMMY.value - self.value;
    }
}

impl crate::prg::FromRng for Dummy {
    fn from_rng(&mut self, rng: &mut impl rand::Rng) {
        RandBigInt::gen_biguint_below(rng, &MODULUS.value);
    }
}

impl crate::Share for Dummy {}

impl crate::Group for u64 {
    #[inline]
    fn zero() -> Self {
        0u64
    }

    #[inline]
    fn one() -> Self {
        1u64
    }

    #[inline]
    fn add(&mut self, other: &Self) {
        debug_assert!(*self < MODULUS_64);
        debug_assert!(*other < MODULUS_64);
        *self += other;
        *self %= MODULUS_64;
    }

    #[inline]
    fn mul(&mut self, other: &Self) {
        debug_assert!(*self < MODULUS_64);
        debug_assert!(*other < MODULUS_64);
        let s64: u64 = *self;
        let o64: u64 = *other;
        let a: u128 = s64.into();
        let b: u128 = o64.into();

        let res = (a * b) % MODULUS_64_BIG;
        *self = res.try_into().unwrap();
    }

    #[inline]
    fn add_lazy(&mut self, other: &Self) {
        self.add(other);
    }

    #[inline]
    fn mul_lazy(&mut self, other: &Self) {
        self.mul(other);
    }

    #[inline]
    fn reduce(&mut self) {}

    #[inline]
    fn sub(&mut self, other: &Self) {
        debug_assert!(*self < MODULUS_64);
        debug_assert!(*other < MODULUS_64);
        let mut neg = *other;
        neg.negate();
        self.add(&neg);
    }

    #[inline]
    fn negate(&mut self) {
        debug_assert!(*self < MODULUS_64);
        *self = MODULUS_64 - *self;
        *self %= MODULUS_64;
    }
}

impl crate::prg::FromRng for u64 {
    fn from_rng(&mut self, rng: &mut impl rand::Rng) {
        *self = u64::MAX;
        while *self >= MODULUS_64 {
            *self = rng.next_u64();
            *self &= 0x7fffffffffffffffu64;
        }
    }
}

impl crate::Share for u64 {}

impl crate::Group for FE {
    #[inline]
    fn zero() -> Self {
        FE::from(0u8)
    }

    #[inline]
    fn one() -> Self {
        FE::from(1u8)
    }

    #[inline]
    fn add(&mut self, other: &Self) {
        use std::ops::Add;
        *self = <FE as Add>::add(*self, *other);
    }

    #[inline]
    fn mul(&mut self, other: &Self) {
        use std::ops::Mul;
        *self = <FE as Mul>::mul(*self, *other);
    }

    #[inline]
    fn add_lazy(&mut self, other: &Self) {
        self.add(other);
    }

    #[inline]
    fn mul_lazy(&mut self, other: &Self) {
        self.mul(other);
    }

    #[inline]
    fn reduce(&mut self) {}

    #[inline]
    fn sub(&mut self, other: &Self) {
        use std::ops::Sub;
        *self = <FE as Sub>::sub(*self, *other);
    }

    #[inline]
    fn negate(&mut self) {
        use std::ops::Neg;
        *self = self.neg();
    }
}

impl crate::prg::FromRng for FE {
    fn from_rng(&mut self, rng: &mut impl rand::Rng) {
        loop {
            let v = FE::from_u64_unbiased(rng.next_u64());
            match v {
                Some(x) => {
                    *self = x;
                    break;
                }
                None => continue,
            }
        }
    }
}

impl crate::Share for FE {}

impl Ord for FE {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value().cmp(&other.value())
    }
}

impl PartialOrd for FE {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.value().cmp(&other.value()))
    }
}

/*******/

impl From<u32> for FieldElm {
    #[inline]
    fn from(inp: u32) -> Self {
        FieldElm {
            value: BigUint::from(inp),
        }
    }
}

impl From<BigUint> for FieldElm {
    #[inline]
    fn from(inp: BigUint) -> Self {
        FieldElm { value: inp }
    }
}

impl Ord for FieldElm {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for FieldElm {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.value.cmp(&other.value))
    }
}

impl crate::Group for FieldElm {
    #[inline]
    fn zero() -> Self {
        FieldElm::from(0)
    }

    #[inline]
    fn one() -> Self {
        FieldElm::from(1)
    }

    #[inline]
    fn add(&mut self, other: &Self) {
        //*self = FieldElm::from((&self.value + &other.value) % &MODULUS.value);
        self.value += &other.value;
        self.value %= &MODULUS.value;
    }

    #[inline]
    fn mul(&mut self, other: &Self) {
        self.value *= &other.value;
        self.value %= &MODULUS.value;
    }

    #[inline]
    fn add_lazy(&mut self, other: &Self) {
        self.value += &other.value;
    }

    #[inline]
    fn mul_lazy(&mut self, other: &Self) {
        self.value *= &other.value;
    }

    #[inline]
    fn reduce(&mut self) {
        self.value %= &MODULUS.value;
    }

    #[inline]
    fn sub(&mut self, other: &Self) {
        // XXX not constant time
        if self.value < other.value {
            self.value += &MODULUS.value;
        }

        *self = FieldElm::from(&self.value - &other.value);
    }

    #[inline]
    fn negate(&mut self) {
        self.value = &MODULUS.value - &self.value;
    }
}

impl crate::prg::FromRng for FieldElm {
    #[inline]
    fn from_rng(&mut self, rng: &mut impl rand::Rng) {
        self.value = rng.gen_biguint_below(&MODULUS.value);
    }
}

impl crate::Share for FieldElm {}

impl<T> crate::Group for (T, T)
where
    T: crate::Group + Clone,
{
    #[inline]
    fn zero() -> Self {
        (T::zero(), T::zero())
    }

    #[inline]
    fn one() -> Self {
        (T::one(), T::one())
    }

    #[inline]
    fn add(&mut self, other: &Self) {
        self.0.add(&other.0);
        self.1.add(&other.1);
    }

    #[inline]
    fn mul(&mut self, other: &Self) {
        self.0.mul(&other.0);
        self.1.mul(&other.1);
    }

    #[inline]
    fn add_lazy(&mut self, other: &Self) {
        self.0.add_lazy(&other.0);
        self.1.add_lazy(&other.1);
    }

    #[inline]
    fn mul_lazy(&mut self, other: &Self) {
        self.0.mul_lazy(&other.0);
        self.1.mul_lazy(&other.1);
    }

    #[inline]
    fn reduce(&mut self) {
        self.0.reduce();
        self.1.reduce();
    }

    #[inline]
    fn negate(&mut self) {
        self.0.negate();
        self.1.negate();
    }

    #[inline]
    fn sub(&mut self, other: &Self) {
        let mut inv0 = other.0.clone();
        let mut inv1 = other.1.clone();
        inv0.negate();
        inv1.negate();
        self.0.add(&inv0);
        self.1.add(&inv1);
    }
}

impl<T> crate::prg::FromRng for (T, T)
where
    T: crate::prg::FromRng + crate::Group,
{
    fn from_rng(&mut self, mut rng: &mut impl rand::Rng) {
        self.0 = T::zero();
        self.1 = T::zero();
        self.0.from_rng(&mut rng);
        self.1.from_rng(&mut rng);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::Group;

    #[test]
    fn add() {
        let mut res = FieldElm::zero();
        let one = FieldElm::from(1);
        let two = FieldElm::from(2);
        res.add(&one);
        res.add(&one);
        assert_eq!(two, res);
    }

    #[test]
    fn add_big() {
        let mut res = FieldElm::zero();
        let two = FieldElm::from(2);
        res.add(&two);
        res.add(&MODULUS);
        assert_eq!(two, res);
    }

    #[test]
    fn mul_big() {
        let mut res = FieldElm::zero();
        let two = FieldElm::from(2);
        res.add(&two);
        res.mul(&MODULUS);
        assert_eq!(res, FieldElm::zero());
    }

    #[test]
    fn mul_big2() {
        let mut res = FieldElm::zero();
        let two = FieldElm::from(2);
        let eight = FieldElm::from(8);
        res.add(&two);
        res.mul(&eight);
        assert_eq!(res, FieldElm::from(16));
    }

    #[test]
    fn negate() {
        let zero = FieldElm::zero();
        let x = FieldElm::from(1123123);
        let mut negx = FieldElm::from(1123123);
        let mut res = FieldElm::zero();

        negx.negate();
        res.add(&x);
        res.add(&negx);
        assert_eq!(zero, res);
    }

    #[test]
    fn rand() {
        let zero = FieldElm::zero();
        let nonzero = FieldElm::random();
        assert!(zero != nonzero);
    }

    #[test]
    fn sub() {
        let zero = FieldElm::zero();
        let mut x = FieldElm::from(1123123);
        let xp = x.clone();
        x.sub(&xp);
        assert_eq!(x, zero);

        let mut y = FieldElm::from(7);
        y.sub(&FieldElm::from(3));
        let exp2 = FieldElm::from(4);
        assert_eq!(y, exp2);
    }

    #[test]
    fn add128() {
        let mut res = u64::zero();
        let one = 1u64;
        let two = 2u64;
        res.add(&one);
        res.add(&one);
        assert_eq!(two, res);
    }

    #[test]
    fn add_big128() {
        let mut res = 1u64;
        let two = 2u64;
        res.add(&two);
        res.add(&(MODULUS_64 - 1));
        assert_eq!(two, res);
    }

    #[test]
    fn mul_big128() {
        let mut res = 0u64;
        let four = 4u64;
        res.add(&four);
        res.mul(&(MODULUS_64 - 1));
        assert_eq!(res, MODULUS_64 - 4);
    }

    #[test]
    fn mul_big2128() {
        let mut res = u64::zero();
        let two = 2u64;
        let eight = 8u64;
        res.add(&two);
        res.mul(&eight);
        assert_eq!(res, 16u64);
    }

    #[test]
    fn negate128() {
        let zero = u64::zero();
        let x = 1123123u64;
        let mut negx = 1123123u64;
        let mut res = 0u64;

        negx.negate();
        res.add(&x);
        res.add(&negx);
        assert_eq!(zero, res);
    }
}

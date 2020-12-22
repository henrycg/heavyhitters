use crate::dpf;
use crate::mpc;

use serde::{Deserialize, Serialize};

pub const TRIPLES_PER_LEVEL: usize = 3;

/// All-prefix DPF supporting protection against additive attacks.
///
/// If the key represents a vector x \in F^n, we encode the key
/// as a vector (a, a^2, x, a.x + a^2), for a random a \in \F.
///
/// TODO Explain how servers validate the sketch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchDPFKey<T, U> {
    pub mac_key: T,
    pub mac_key2: T,
    pub mac_key_last: U,
    pub mac_key2_last: U,
    key: dpf::DPFKey<(T, T), (U, U)>,

    pub triples: Vec<mpc::TripleShare<T>>,
    pub triples_last: Vec<mpc::TripleShare<U>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SketchOutput<T> {
    // Compute
    //          <r, x>
    //          <r^2, x>
    //          <r, k.x> + k^2
    //          <r^2, k.x> + k^2
    pub r_x: T,
    pub r2_x: T,
    pub r_kx: T,

    // Random values shared between the two
    // servers for taking a linear combination
    // of the sketch outputs.
    pub rand1: T,
    pub rand2: T,
    pub rand3: T,
}

impl<T> SketchOutput<T>
where
    T: crate::Group,
{
    pub fn zero() -> Self {
        SketchOutput {
            r_x: T::zero(),
            r2_x: T::zero(),
            r_kx: T::zero(),

            rand1: T::zero(),
            rand2: T::zero(),
            rand3: T::zero(),
        }
    }

    pub fn add(&mut self, other: &Self) {
        self.r_x.add(&other.r_x);
        self.r2_x.add(&other.r2_x);
        self.r_kx.add(&other.r_kx);
    }

    pub fn reduce(&mut self) {
        self.r_x.reduce();
        self.r2_x.reduce();
        self.r_kx.reduce();
    }
}

impl<T,U> SketchDPFKey<T,U>
where
    T: crate::Share + std::fmt::Debug + std::cmp::PartialEq,
    U: crate::Share + std::fmt::Debug + std::cmp::PartialEq,
{
    #[allow(clippy::needless_range_loop)]
    pub fn gen(alpha_bits: &[bool], values_in: &[T], value_last: &U) -> [SketchDPFKey<T,U>; 2] {
        // For MAC key a, encode data as
        //      (a, a^2, x, a.x).
        let mac_key = T::random();
        let (mac_key_sh0, mac_key_sh1) = mac_key.share();

        let mut mac_key2 = mac_key.clone();
        mac_key2.mul(&mac_key);
        let (mac_key2_sh0, mac_key2_sh1) = mac_key2.share();

        // Need a separate MAC key for last level of tree.
        let mac_key_last = U::random();
        let (mac_key_sh0_last, mac_key_sh1_last) = mac_key_last.share();

        let mut mac_key2_last = mac_key_last.clone();
        mac_key2_last.mul(&mac_key_last);
        let (mac_key2_sh0_last, mac_key2_sh1_last) = mac_key2_last.share();

        let mut values = Vec::new();
        for i in 0..alpha_bits.len()-1 {
            // Compute (x, a.x)
            let mut mac_val = values_in[i].clone();
            mac_val.mul(&mac_key);
            values.push((values_in[i].clone(), mac_val));
        }

        let mut mac_val_last = value_last.clone();
        mac_val_last.mul(&mac_key_last);
        let value_last_with_mac = (value_last.clone(), mac_val_last);

        let (dpf_key0, dpf_key1) = dpf::DPFKey::gen(alpha_bits, &values, &value_last_with_mac);

        let mut triples0 = vec![];
        let mut triples1 = vec![];
        for _i in 0..TRIPLES_PER_LEVEL * (alpha_bits.len() - 1) {
            let t = mpc::TripleShare::new();
            triples0.push(t[0].clone());
            triples1.push(t[1].clone());
        }

        let mut triples0_last = vec![];
        let mut triples1_last = vec![];
        for _i in 0..TRIPLES_PER_LEVEL {
            let t = mpc::TripleShare::new();
            triples0_last.push(t[0].clone());
            triples1_last.push(t[1].clone());
        }


        [
            SketchDPFKey {
                mac_key: mac_key_sh0,
                mac_key2: mac_key2_sh0,
                mac_key_last: mac_key_sh0_last,
                mac_key2_last: mac_key2_sh0_last,
                key: dpf_key0,
                triples: triples0,
                triples_last: triples0_last
            },
            SketchDPFKey {
                mac_key: mac_key_sh1,
                mac_key2: mac_key2_sh1,
                mac_key_last: mac_key_sh1_last,
                mac_key2_last: mac_key2_sh1_last,
                key: dpf_key1,
                triples: triples1,
                triples_last: triples1_last
            },
        ]
    }

    pub fn gen_from_str(s: &str) -> [SketchDPFKey<T,U>; 2] {
        let bits = crate::string_to_bits(s);
        let values = vec![T::one(); bits.len()-1];
        SketchDPFKey::gen(&bits, &values, &U::one())
    }

    pub fn sketch_at(
        &self,
        vector_in: &[(T, T)],
        rand_stream: &mut impl rand::Rng,
    ) -> SketchOutput<T> {
        let mut out: SketchOutput<T> = SketchOutput::zero();

        out.rand1.from_rng(rand_stream);
        out.rand2.from_rng(rand_stream);
        out.rand3.from_rng(rand_stream);

        for v in vector_in {
            // Get r_i from PRG stream
            let mut sketch_r = T::zero();
            sketch_r.from_rng(rand_stream);

            // Compute r_i^2
            let mut sketch_r2 = sketch_r.clone();
            sketch_r2.mul_lazy(&sketch_r);

            // Compute
            //          <r, x>
            //          <r^2, x>
            //          <r, k.x> 

            let (x, kx) = v;

            let mut tmp0 = x.clone();
            tmp0.mul_lazy(&sketch_r);

            let mut tmp1 = x.clone();
            tmp1.mul_lazy(&sketch_r2);

            let mut tmp2 = kx.clone();
            tmp2.mul_lazy(&sketch_r);

            out.r_x.add_lazy(&tmp0);
            out.r2_x.add_lazy(&tmp1);
            out.r_kx.add_lazy(&tmp2);
        }

        out.reduce();
        out
    }

    pub fn sketch_at_last(
        &self,
        vector_in: &[(U, U)],
        rand_stream: &mut impl rand::Rng,
    ) -> SketchOutput<U> {
        let mut out: SketchOutput<U> = SketchOutput::zero();

        out.rand1.from_rng(rand_stream);
        out.rand2.from_rng(rand_stream);
        out.rand3.from_rng(rand_stream);

        for v in vector_in {
            // Get r_i from PRG stream
            let mut sketch_r = U::zero();
            sketch_r.from_rng(rand_stream);

            // Compute r_i^2
            let mut sketch_r2 = sketch_r.clone();
            sketch_r2.mul_lazy(&sketch_r);

            // Compute
            //          <r, x>
            //          <r^2, x>
            //          <r, k.x>

            let (x, kx) = v;

            let mut tmp0 = x.clone();
            tmp0.mul_lazy(&sketch_r);

            let mut tmp1 = x.clone();
            tmp1.mul_lazy(&sketch_r2);

            let mut tmp2 = kx.clone();
            tmp2.mul_lazy(&sketch_r);

            out.r_x.add_lazy(&tmp0);
            out.r2_x.add_lazy(&tmp1);
            out.r_kx.add_lazy(&tmp2);
        }

        out.reduce();
        out
    }


    pub fn eval(&self, idx: &[bool]) -> U {
        debug_assert!(idx.len() <= self.key.domain_size()+1);
        debug_assert!(!idx.is_empty());

        (self.key.eval(idx).1).1
    }

    pub fn eval_bit(&self, state: &dpf::EvalState, dir: bool) -> (dpf::EvalState, T, T) {
        let (st, val) = self.key.eval_bit(state, dir);
        (st, val.0, val.1)
    }

    pub fn eval_bit_last(&self, state: &dpf::EvalState, dir: bool) -> (dpf::EvalState, U, U) {
        let (st, val) = self.key.eval_bit_last(state, dir);
        (st, val.0, val.1)
    }

    pub fn eval_init(&self) -> dpf::EvalState {
        self.key.eval_init()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::FieldElm;
    use crate::Group;

    #[test]
    fn sketch_add() {
        let a = SketchOutput {
            r_x: FieldElm::from(3),
            r2_x: FieldElm::from(4),
            r_kx: FieldElm::from(5),

            rand1: FieldElm::from(0),
            rand2: FieldElm::from(0),
            rand3: FieldElm::from(0),
        };

        let mut b = SketchOutput::<FieldElm>::zero();
        b.add(&a);

        assert_eq!(a, b);

        b.add(&a);
        assert_eq!(b.r_x, FieldElm::from(6));
        assert_eq!(b.r2_x, FieldElm::from(8));
        assert_eq!(b.r_kx, FieldElm::from(10));
    }

    #[test]
    fn mac_keys() {
        let nbits = 3;
        let alpha = crate::u32_to_bits(nbits, 3);
        let betas = vec![
            FieldElm::from(7u32),
            FieldElm::from(17u32),
        ];
        let beta_last = FieldElm::from(2u32);
        let keys = SketchDPFKey::gen(&alpha, &betas, &beta_last);

        let mut mac = FieldElm::zero();
        let mut mac2 = FieldElm::zero();

        for i in 0..2 {
            mac.add(&keys[i].mac_key);
            mac2.add(&keys[i].mac_key2);
        }

        println!("mac  = {:?}", mac);
        println!("mac2 = {:?}", mac2);
        mac.mul(&mac.clone());
        assert_eq!(mac, mac2);
    }

    #[test]
    fn mac_value() {
        let nbits = 3;
        let alpha = crate::u32_to_bits(nbits, 3);
        let betas = vec![
            FieldElm::from(7u32),
            FieldElm::from(17u32),
        ];
        let beta_last = FieldElm::from(2u32);
        let keys = SketchDPFKey::gen(&alpha, &betas, &beta_last);

        let mut mac = FieldElm::zero();
        let mut mac2 = FieldElm::zero();

        for i in 0..2 {
            mac.add(&keys[i].mac_key);
            mac2.add(&keys[i].mac_key2);
        }

        for i in 0..(1 << nbits)-1 {
            let alpha_eval = crate::u32_to_bits(nbits, i);

            println!("Alpha: {:?}", alpha);
            for j in 0..((nbits-1) as usize) {
                if j < 2 {
                    continue;
                }

                let eval0 = keys[0].key.eval(&alpha_eval[0..j].to_vec());
                let eval1 = keys[1].key.eval(&alpha_eval[0..j].to_vec());

                assert_eq!(eval0.0.len(), j-1);
                assert_eq!(eval1.0.len(), j-1);

                let mut tmp0 = FieldElm::zero();
                tmp0.add(&eval0.0[j - 1].0);
                tmp0.add(&eval1.0[j - 1].0);

                let mut tmp1 = FieldElm::zero();
                tmp1.add(&eval0.0[j - 1].1);
                tmp1.add(&eval1.0[j - 1].1);
                tmp1.add(&keys[0].mac_key2);
                tmp1.add(&keys[1].mac_key2);

                // Should be that
                //   mac*tmp0 + mac2 = tmp1
                let mut shouldbe = tmp0.clone();
                shouldbe.mul(&mac);
                shouldbe.add(&mac2);
                assert_eq!(shouldbe, tmp1);
            }
        }
    }
}

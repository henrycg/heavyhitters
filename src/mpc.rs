use crate::sketch;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TripleShare<T> {
    pub a: T,
    pub b: T,
    pub c: T,
}

// XXX: Optimization: compress Beaver triples.
impl<T> TripleShare<T>
where
    T: crate::Share + std::fmt::Debug,
{
    pub fn new() -> [TripleShare<T>; 2] {
        let (a_s0, a_s1) = T::share_random();
        let (b_s0, b_s1) = T::share_random();

        // c = a*b
        let mut c = a_s0.clone();
        c.add(&a_s1);

        let mut b = b_s0.clone();
        b.add(&b_s1);

        c.mul(&b);

        let (c_s0, c_s1) = c.share();

        [
            TripleShare {
                a: a_s0,
                b: b_s0,
                c: c_s0,
            },
            TripleShare {
                a: a_s1,
                b: b_s1,
                c: c_s1,
            },
        ]
    }
}

// We will compute in MPC:
//    \sum_i [ (x_i * y_i) + z_i ]
#[derive(Clone)]
pub struct MulState<T> {
    server_idx: bool,
    triples: Vec<TripleShare<T>>,

    xs: Vec<T>,
    ys: Vec<T>,
    zs: Vec<T>,

    rs: Vec<T>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorShare<T> {
    ds: Vec<T>,
    es: Vec<T>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cor<T> {
    ds: Vec<T>,
    es: Vec<T>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutShare<T> {
    share: T,
}

impl<T> MulState<T>
where
    T: crate::Share + std::cmp::PartialEq + std::fmt::Debug + From<u32>,
{
    pub fn new(
        server_idx: bool,
        triples: Vec<TripleShare<T>>,
        mac_key: &T,
        mac_key2: &T,
        sketch: &sketch::SketchOutput<T>,
        level: usize,
    ) -> MulState<T> {
        debug_assert!((level + 1) * sketch::TRIPLES_PER_LEVEL <= triples.len());

        let trip_start = level * sketch::TRIPLES_PER_LEVEL;
        let trip_end = trip_start + sketch::TRIPLES_PER_LEVEL;

        let mut out = MulState {
            server_idx,
            triples: triples[trip_start..trip_end].to_vec(),

            xs: Vec::with_capacity(sketch::TRIPLES_PER_LEVEL),
            ys: Vec::with_capacity(sketch::TRIPLES_PER_LEVEL),
            zs: Vec::with_capacity(sketch::TRIPLES_PER_LEVEL),

            rs: Vec::with_capacity(sketch::TRIPLES_PER_LEVEL),
        };

        // 1) Check original sketch would have accepted.
        //      <r,x>^2 - <r^2,x> =? 0
        out.xs.push(sketch.r_x.clone());
        out.ys.push(sketch.r_x.clone());

        let mut c0 = sketch.r2_x.clone();
        c0.negate();
        out.zs.push(c0);

        // 2) Check MAC values are correct.
        //    For linear query q, vector x, MAC key k
        //          (<q, kx> + k^2) - k^2 - k*<q,x> == 0?

        //   2a) Check that k^2 - k*k = 0
        let mut mac_key2_neg = mac_key2.clone();
        mac_key2_neg.negate();

        out.xs.push(mac_key.clone());
        out.ys.push(mac_key.clone());
        out.zs.push(mac_key2_neg);

        //   2b) Check k <r,x> - <r, kx> = 0
        out.xs.push(sketch.r_x.clone());
        out.ys.push(mac_key.clone());

        let mut sketch_r_kx_neg = sketch.r_kx.clone();
        sketch_r_kx_neg.negate(); 
        out.zs.push(sketch_r_kx_neg);

        out.rs = vec![sketch.rand1.clone(), 
                    sketch.rand2.clone(), 
                    sketch.rand3.clone()];

        out
    }

    pub fn cor_share(&self) -> CorShare<T> {
        let mut out = CorShare {
            ds: Vec::with_capacity(sketch::TRIPLES_PER_LEVEL),
            es: Vec::with_capacity(sketch::TRIPLES_PER_LEVEL),
        };

        for i in 0..sketch::TRIPLES_PER_LEVEL {
            let mut d = self.xs[i].clone();
            d.sub(&self.triples[i].a);
            out.ds.push(d);

            let mut e = self.ys[i].clone();
            e.sub(&self.triples[i].b);
            out.es.push(e);
        }

        out
    }

    pub fn cor(share0: &CorShare<T>, share1: &CorShare<T>) -> Cor<T> {
        let mut out = Cor {
            ds: Vec::with_capacity(sketch::TRIPLES_PER_LEVEL),
            es: Vec::with_capacity(sketch::TRIPLES_PER_LEVEL),
        };

        for i in 0..sketch::TRIPLES_PER_LEVEL {
            let mut d = T::zero();
            d.add(&share0.ds[i]);
            d.add(&share1.ds[i]);
            out.ds.push(d);

            let mut e = T::zero();
            e.add(&share0.es[i]);
            e.add(&share1.es[i]);
            out.es.push(e);
        }

        out
    }

    pub fn out_share(&self, cor: &Cor<T>) -> OutShare<T> {
        let mut out = T::zero();
        for i in 0..sketch::TRIPLES_PER_LEVEL {
            let mut term = T::zero();

            // Compute
            // d*e/2 + d*b_i + e*a_i + c_i + z_i
            if self.server_idx {
                // Add in d*e to first share only
                let mut tmp = cor.ds[i].clone();
                tmp.mul_lazy(&cor.es[i]);

                term.add_lazy(&tmp);
            }

            let mut tmp = cor.ds[i].clone();
            tmp.mul_lazy(&self.triples[i].b);
            term.add_lazy(&tmp);

            tmp = cor.es[i].clone();
            tmp.mul_lazy(&self.triples[i].a);
            term.add_lazy(&tmp);

            term.add_lazy(&self.triples[i].c);

            term.add_lazy(&self.zs[i]);
            term.mul_lazy(&self.rs[i]);
            out.add_lazy(&term);
        }

        out.reduce();
        OutShare { share: out }
    }

    pub fn verify(out0: &OutShare<T>, out1: &OutShare<T>) -> bool {
        let mut val = out0.share.clone();
        val.add(&out1.share);

        val == T::zero()
    }
}

/// Verify an array of SketchOutput<T>'s in parallel.
#[derive(Clone)]
pub struct ManyMulState<T> {
    states: Vec<MulState<T>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManyCorShare<T> {
    cor_shares: Vec<CorShare<T>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManyCor<T> {
    cors: Vec<Cor<T>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManyOutShare<T> {
    out_shares: Vec<OutShare<T>>,
}

impl<T> ManyMulState<T>
where
    T: crate::Share + std::cmp::PartialEq + std::fmt::Debug + From<u32>,
{
    pub fn zero() -> ManyMulState<T> {
        ManyMulState { states: vec![] }
    }

    pub fn new(
        server_idx: bool,
        triples: &[Vec<TripleShare<T>>],
        mac_keys: &[T],
        mac_keys2: &[T],
        sketch: &[sketch::SketchOutput<T>],
        level: usize,
    ) -> ManyMulState<T> {
        debug_assert_eq!(triples.len(), sketch.len());

        let mut out = ManyMulState {
            states: Vec::with_capacity(triples.len()),
        };
        for i in 0..triples.len() {
            out.states
                .push(MulState::new(server_idx, triples[i].clone(), &mac_keys[i], &mac_keys2[i], &sketch[i], level));
        }

        out
    }

    pub fn cor_shares(&self) -> ManyCorShare<T> {
        let mut out = ManyCorShare {
            cor_shares: Vec::with_capacity(self.states.len()),
        };
        for i in 0..self.states.len() {
            out.cor_shares.push(self.states[i].cor_share());
        }
        out
    }

    pub fn cors(s0: &ManyCorShare<T>, s1: &ManyCorShare<T>) -> ManyCor<T> {
        debug_assert_eq!(s0.cor_shares.len(), s1.cor_shares.len());

        let mut out = ManyCor {
            cors: Vec::with_capacity(s0.cor_shares.len()),
        };

        for i in 0..s0.cor_shares.len() {
            out.cors
                .push(MulState::cor(&s0.cor_shares[i], &s1.cor_shares[i]));
        }
        out
    }

    pub fn out_shares(&self, cor: &ManyCor<T>) -> ManyOutShare<T> {
        debug_assert_eq!(cor.cors.len(), self.states.len());
        let mut out = ManyOutShare {
            out_shares: Vec::with_capacity(self.states.len()),
        };
        for (i, st) in self.states.iter().enumerate() {
            out.out_shares.push(st.out_share(&cor.cors[i]));
        }
        out
    }

    pub fn verify(out0: &ManyOutShare<T>, out1: &ManyOutShare<T>) -> Vec<bool> {
        debug_assert_eq!(out0.out_shares.len(), out1.out_shares.len());
        let mut out = Vec::with_capacity(out0.out_shares.len());
        for i in 0..out0.out_shares.len() {
            out.push(MulState::verify(&out0.out_shares[i], &out1.out_shares[i]));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FieldElm;
    use crate::Group;

    #[test]
    fn triple() {
        let [t0, t1] = TripleShare::<FieldElm>::new();

        debug_assert!(t0.a != FieldElm::zero());
        debug_assert!(t0.b != FieldElm::zero());
        debug_assert!(t0.c != FieldElm::zero());

        let mut a = t0.a.clone();
        a.add(&t1.a);

        let mut b = t0.b.clone();
        b.add(&t1.b);

        let mut c = t0.c.clone();
        c.add(&t1.c);

        let mut ab = a.clone();
        ab.mul(&b);

        assert_eq!(ab, c);
    }
}

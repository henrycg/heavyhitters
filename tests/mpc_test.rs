/*
use counttree::prg::PrgSeed;
use counttree::sketch::*;
use counttree::*;



#[test]
fn mpc_test() {
    let nbits = 5;
    let alpha = u32_to_bits(nbits, 21);
    let betas = vec![
        FieldElm::from(1u32),
        FieldElm::from(1u32),
        FieldElm::from(1u32),
        FieldElm::from(1u32),
    ];
    let beta_last = FieldElm::from(1u32);
    let keys = SketchDPFKey::gen(&alpha, &betas, &beta_last);

    for level in 1..nbits {
        let seed = PrgSeed::random();

        let mut range = vec![];
        for k in 0..(1 << level) {
            if k % 3 != 0 {
                range.push(u32_to_bits(level, k));
            }
        }

        let mut sketches = vec![];
        for j in 0..2 {
            sketches.push(keys[j].sketch_at(&range, &mut seed.to_rng()));
        }

        let mut triples0 = vec![];
        let mut triples1 = vec![];
        for _i in 0..3 {
            let [t0, t1]: [mpc::TripleShare<FieldElm>; 2] = mpc::TripleShare::new();
            triples0.push(t0);
            triples1.push(t1);
        }

        let level_zero: usize = (level - 1).into();
        let state0 = mpc::MulState::new(false, keys[0].triples.clone(), &keys[0].mac_key, &keys[0].mac_key2, &sketches[0], level_zero);
        let state1 = mpc::MulState::new(true, keys[1].triples.clone(), &keys[1].mac_key, &keys[1].mac_key2, &sketches[1], level_zero);

        let mut k = FieldElm::zero();
        k.add(&keys[0].mac_key);
        k.add(&keys[1].mac_key);

        let mut k2 = FieldElm::zero();
        k2.add(&keys[0].mac_key2);
        k2.add(&keys[1].mac_key2);

        let mut tmp = k.clone();
        tmp.mul(&k);
        assert_eq!(k2, tmp);

        let corshare0 = state0.cor_share();
        let corshare1 = state1.cor_share();

        let cor = mpc::MulState::cor(&corshare0, &corshare1);

        let outshare0 = state0.out_share(&cor);
        let outshare1 = state1.out_share(&cor);

        assert!(mpc::MulState::verify(&outshare0, &outshare1));
    }
}
*/

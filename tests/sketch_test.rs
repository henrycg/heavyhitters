/*
use counttree::prg::PrgSeed;
use counttree::sketch::*;
use counttree::Group;
use counttree::*;

use std::collections::HashMap;
use std::collections::VecDeque;

#[test]
fn sketch_eval() {
    let nbits = 5;
    let alpha = u32_to_bits(nbits, 21);
    let betas = vec![
        FieldElm::from(7u32),
        FieldElm::from(17u32),
        FieldElm::from(2u32),
        FieldElm::from(0u32),
    ];
    let beta_last = FieldElm::from(32u32);
    let keys = SketchDPFKey::gen(&alpha, &betas, &beta_last);

    for i in 0..(1 << nbits) {
        let alpha_eval = u32_to_bits(nbits, i);

        println!("Alpha: {:?}", alpha);
        for j in 0..(nbits as usize) {
            if j == 0 {
                continue;
            }

            let eval0 = keys[0].eval(&alpha_eval[0..j].to_vec());
            let eval1 = keys[1].eval(&alpha_eval[0..j].to_vec());
            let mut tmp = FieldElm::zero();

            tmp.add(&eval0);
            tmp.add(&eval1);
            println!("[{:?}] Tmp {:?} = {:?}", alpha_eval, j, tmp);
            if alpha[0..j] == alpha_eval[0..j] {
                assert_eq!(
                    betas[j - 1],
                    tmp,
                    "[Level {:?}] Value incorrect at {:?}",
                    j,
                    alpha_eval
                );
            } else {
                assert_eq!(FieldElm::zero(), tmp);
            }
        }
    }
}
*/

/*
#[test]
fn sketch_verify() {
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
            sketches.push(keys[j].eval_and_sketch_at(&range, &mut seed.to_rng()));
        }

        assert!(insecure_verify(
            &sketches[0],
            (&keys[0].mac_key, &keys[0].mac_key2),
            &sketches[1],
            (&keys[1].mac_key, &keys[1].mac_key2)
        ));
    }
}

fn eval_keys_at(
    keys0: &[SketchDPFKey<FieldElm,FieldElm>],
    keys1: &[SketchDPFKey<FieldElm,FieldElm>],
    eval_at: &[bool],
) -> FieldElm {
    let mut out = FieldElm::zero();
    for k in keys0 {
        out.add(&k.eval(eval_at));
    }

    for k in keys1 {
        out.add(&k.eval(eval_at));
    }

    out
}

fn check_mac(k: &FieldElm, k2: &FieldElm, qx: &FieldElm, q_kx: &FieldElm) -> bool {
    // Check
    //      (<q, kx> + k^2) - k^2 - k*<q, x> == 0

    // <q, kx + k^2>
    let mut out = q_kx.clone();

    // subtract k^2
    out.sub(&k2);

    // subtract <k, qx>
    let mut tmp = qx.clone();
    tmp.mul(&k);

    out.sub(&tmp);

    out == FieldElm::zero()
}

pub fn insecure_verify(
    s0: &SketchOutput<FieldElm>,
    keys0: (&FieldElm, &FieldElm),
    s1: &SketchOutput<FieldElm>,
    keys1: (&FieldElm, &FieldElm),
) -> bool {
    let mut out: SketchOutput<FieldElm> = SketchOutput::zero();
    out.add(&s0);
    out.add(&s1);

    let mut k = FieldElm::zero();
    k.add(&keys0.0);
    k.add(&keys1.0);

    let mut k2 = FieldElm::zero();
    k2.add(&keys0.1);
    k2.add(&keys1.1);

    // 1) Check original sketch would have accepted.
    //      <r,x>^2 - <r^2,x> =? 0
    let mut tmp0 = out.r_x.clone();
    tmp0.mul(&out.r_x);
    tmp0.sub(&out.r2_x);
    let good0 = tmp0 == FieldElm::zero();

    // 2) Check MAC values are correct.

    //   2a) Check <r,x> computed correctly.
    //          <r,kx>^2  - k^2.<r, (1,...,1)> - k.<r,x>
    let good1 = check_mac(&k, &k2, &out.r_x, &out.r_kx);

    good0 && good1
}


#[test]
fn traverse_test_eval_slow() {
    let client_strings = [
        "abdef", "abdef", "abdef", "ghijk", "gZijk", "gZ???", "  ?*g", "abdef", "gZ???", "gZ???",
    ];

    let strlen = crate::string_to_bits(&client_strings[0]).len();
    let mut keys0 = vec![];
    let mut keys1 = vec![];

    for cstr in &client_strings {
        let keys = SketchDPFKey::<FieldElm,FieldElm>::gen_from_str(&cstr);
        keys0.push(keys[0].clone());
        keys1.push(keys[1].clone());
    }

    let mut strings = VecDeque::new();
    strings.push_back(vec![]);

    // Use breadth-first search
    let mut out = HashMap::new();
    while strings.len() > 0 {
        let mut eval_at: Vec<bool> = strings.pop_front().unwrap();
        if eval_at.len() == strlen {
            let val = eval_keys_at(&keys0, &keys1, &eval_at);
            out.insert(eval_at, val);
            continue;
        }

        eval_at.push(false);
        let val0 = eval_keys_at(&keys0, &keys1, &eval_at);

        if val0 > FieldElm::zero() {
            strings.push_back(eval_at.clone());
        }

        eval_at.pop();
        eval_at.push(true);
        let val1 = eval_keys_at(&keys0, &keys1, &eval_at);
        if val1 > FieldElm::zero() {
            strings.push_back(eval_at);
        }
    }

    for (arr, v) in &out {
        let s = crate::bits_to_string(&arr);
        println!("s: {:?} = {:?}", s, v);

        match &s[..] {
            "gZijk" => assert_eq!(*v, FieldElm::from(1)),
            "abdef" => assert_eq!(*v, FieldElm::from(4)),
            "ghijk" => assert_eq!(*v, FieldElm::from(1)),
            "gZ???" => assert_eq!(*v, FieldElm::from(3)),
            "  ?*g" => assert_eq!(*v, FieldElm::from(1)),
            _ => {
                println!("Unexpected string: '{:?}' = {:?}", s, v);
                assert!(false);
            }
        }
    }
}
*/

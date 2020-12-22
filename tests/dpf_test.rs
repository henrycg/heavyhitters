use counttree::dpf::*;
use counttree::*;

#[test]
fn dpf_complete() {
    let nbits = 5;
    let alpha = u32_to_bits(nbits, 21);
    let betas = vec![
        FieldElm::from(7u32),
        FieldElm::from(17u32),
        FieldElm::from(2u32),
        FieldElm::from(0u32),
    ];
    let beta_last = fastfield::FE::from(32u32);
    let (key0, key1) = DPFKey::gen(&alpha, &betas, &beta_last);

    for i in 0..(1 << nbits) {
        let alpha_eval = u32_to_bits(nbits, i);

        println!("Alpha: {:?}", alpha);
        for j in 0..((nbits-1) as usize) {
            if j < 2 {
                continue;
            }

            let eval0 = key0.eval(&alpha_eval[0..j].to_vec());
            let eval1 = key1.eval(&alpha_eval[0..j].to_vec());
            let mut tmp = FieldElm::zero();

            tmp.add(&eval0.0[j - 2]);
            tmp.add(&eval1.0[j - 2]);
            println!("[{:?}] Tmp {:?} = {:?}", alpha_eval, j, tmp);
            if alpha[0..j-1] == alpha_eval[0..j-1] {
                assert_eq!(
                    betas[j - 2],
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

use counttree::collect::*;
use counttree::prg;
use counttree::sketch::*;
use counttree::*;

#[test]
fn collect_test_eval() {
    let client_strings = [
        "abdef", "abdef", "abdef", "ghijk", "gZijk", "gZ???", "  ?*g", "abdef", "gZ???", "gZ???",
    ];

    let strlen = crate::string_to_bits(&client_strings[0]).len();

    let seed = prg::PrgSeed::random();
    let mut col0 = KeyCollection::new(&seed, strlen);
    let mut col1 = KeyCollection::new(&seed, strlen);

    for cstr in &client_strings {
        let keys = SketchDPFKey::<FieldElm,FieldElm>::gen_from_str(&cstr);
        col0.add_key(keys[0].clone());
        col1.add_key(keys[1].clone());
    }

    col0.tree_init();
    col1.tree_init();

    let nclients = client_strings.len();
    let threshold = FieldElm::from(2);
    for level in 0..strlen-1 {
        println!("At level {:?}", level);
        let vals0 = col0.tree_crawl();
        let vals1 = col1.tree_crawl();

        assert_eq!(vals0.len(), vals1.len());
        let keep = KeyCollection::<FieldElm,FieldElm>::keep_values(nclients, &threshold, &vals0, &vals1);

        col0.tree_prune(&keep);
        col1.tree_prune(&keep);
    }

    let vals0 = col0.tree_crawl_last();
    let vals1 = col1.tree_crawl_last();

    assert_eq!(vals0.len(), vals1.len());
    let keep = KeyCollection::<FieldElm,FieldElm>::keep_values_last(nclients, &threshold, &vals0, &vals1);

    col0.tree_prune_last(&keep);
    col1.tree_prune_last(&keep);

    let s0 = col0.final_shares();
    let s1 = col1.final_shares();

    for res in &KeyCollection::<FieldElm,FieldElm>::final_values(&s0, &s1) {
        println!("Path = {:?}", res.path);
        let s = crate::bits_to_string(&res.path);
        println!("fast: {:?} = {:?}", s, res.value);

        match &s[..] {
            "abdef" => assert_eq!(res.value, FieldElm::from(4)),
            "gZ???" => assert_eq!(res.value, FieldElm::from(3)),
            _ => {
                println!("Unexpected string: '{:?}' = {:?}", s, res.value);
                assert!(false);
            }
        }
    }
}

fn verify_sketches(
    col0: &mut KeyCollection<FieldElm,fastfield::FE>,
    col1: &mut KeyCollection<FieldElm,fastfield::FE>,
    level: usize,
    nkeys: usize
) -> Vec<bool> {
    println!("   frontier");
    let sketch0 = col0.tree_sketch_frontier(0, nkeys);
    let sketch1 = col1.tree_sketch_frontier(0, nkeys);
    println!("   done");

    println!("   mul");

    let mut triples0 = vec![];
    let mut mac0 = vec![];
    let mut macp0 = vec![];

    for key in &col0.keys {
        triples0.push(key.1.triples.clone());
        mac0.push(key.1.mac_key.clone());
        macp0.push(key.1.mac_key2.clone());
    }

    let mut triples1 = vec![];
    let mut mac1 = vec![];
    let mut macp1 = vec![];

    for key in &col1.keys {
        triples1.push(key.1.triples.clone());
        mac1.push(key.1.mac_key.clone());
        macp1.push(key.1.mac_key2.clone());
    }

    let many_mul0 = mpc::ManyMulState::new(false, &triples0, &mac0, &macp0, &sketch0, level);
    let many_mul1 = mpc::ManyMulState::new(true, &triples1, &mac1, &macp1, &sketch1, level);

    let cor_shares0 = many_mul0.cor_shares();
    let cor_shares1 = many_mul1.cor_shares();

    let cor = mpc::ManyMulState::cors(&cor_shares0, &cor_shares1);

    let out_shares0 = many_mul0.out_shares(&cor);
    let out_shares1 = many_mul1.out_shares(&cor);

    let out = mpc::ManyMulState::verify(&out_shares0, &out_shares1);
    println!("   done");

    out
}

fn verify_sketches_last(
    col0: &mut KeyCollection<FieldElm,fastfield::FE>,
    col1: &mut KeyCollection<FieldElm,fastfield::FE>,
    nkeys: usize
) -> Vec<bool> {
    println!("   frontier");
    let sketch0 = col0.tree_sketch_frontier_last(0, nkeys);
    let sketch1 = col1.tree_sketch_frontier_last(0, nkeys);
    println!("   done");

    println!("   mul");

    let mut triples0 = vec![];
    let mut mac0 = vec![];
    let mut macp0 = vec![];

    for key in &col0.keys {
        triples0.push(key.1.triples_last.clone());
        mac0.push(key.1.mac_key_last.clone());
        macp0.push(key.1.mac_key2_last.clone());
    }

    let mut triples1 = vec![];
    let mut mac1 = vec![];
    let mut macp1 = vec![];

    for key in &col1.keys {
        triples1.push(key.1.triples_last.clone());
        mac1.push(key.1.mac_key_last.clone());
        macp1.push(key.1.mac_key2_last.clone());
    }

    let many_mul0 = mpc::ManyMulState::new(false, &triples0, &mac0, &macp0, &sketch0, 0);
    let many_mul1 = mpc::ManyMulState::new(true, &triples1, &mac1, &macp1, &sketch1, 0);

    let cor_shares0 = many_mul0.cor_shares();
    let cor_shares1 = many_mul1.cor_shares();

    let cor = mpc::ManyMulState::cors(&cor_shares0, &cor_shares1);

    let out_shares0 = many_mul0.out_shares(&cor);
    let out_shares1 = many_mul1.out_shares(&cor);

    let out = mpc::ManyMulState::verify(&out_shares0, &out_shares1);
    println!("   done");

    out
}

#[test]
fn collect_test_eval_full() {
    let client_strings = [
        "01234567012345670123456701234567",
        "z12x45670y2345670123456701234567",
        "612x45670y2345670123456701234567",
        "912x45670y2345670123456701234567",
    ];

    let nclients = 10;
    let strlen = crate::string_to_bits(&client_strings[0]).len();

    let seed = prg::PrgSeed::random();
    let mut col0 = KeyCollection::new(&seed, strlen);
    let mut col1 = KeyCollection::new(&seed, strlen);
    // use cpuprofiler::PROFILER;

    let mut keys = vec![];
    println!("Starting to generate keys");
    for s in &client_strings {
        keys.push(SketchDPFKey::<FieldElm,fastfield::FE>::gen_from_str(&s));
    }
    println!("Done generating keys");

    for i in 0..nclients {
        let copy0 = keys[i % keys.len()][0].clone();
        let copy1 = keys[i % keys.len()][1].clone();
        col0.add_key(copy0);
        col1.add_key(copy1);
        if i % 50 == 0 {
            println!("  Key {:?}", i);
        }
    }

    col0.tree_init();
    col1.tree_init();

    // PROFILER.lock().unwrap().start("./sketch-2.profile").unwrap();
    let threshold = FieldElm::from(2);
    let threshold_last = fastfield::FE::new(2);
    for level in 0..strlen-1 {
        println!("...start");
        let vals0 = col0.tree_crawl();
        let vals1 = col1.tree_crawl();
        println!("...done");
        println!("At level {:?} (size: {:?})", level, vals0.len());

        println!("...sketch");
        for v in verify_sketches(&mut col0, &mut col1, level, nclients) {
            assert!(v);
        }
        println!("...done");

        assert_eq!(vals0.len(), vals1.len());
        let keep = KeyCollection::<FieldElm,fastfield::FE>::keep_values(nclients, &threshold, &vals0, &vals1);

        col0.tree_prune(&keep);
        col1.tree_prune(&keep);
    }
    // PROFILER.lock().unwrap().stop().unwrap();

    let vals0 = col0.tree_crawl_last();
    let vals1 = col1.tree_crawl_last();

    for v in verify_sketches_last(&mut col0, &mut col1, nclients) {
        assert!(v);
    }

    assert_eq!(vals0.len(), vals1.len());
    let keep = KeyCollection::<FieldElm,fastfield::FE>::keep_values_last(nclients, &threshold_last, &vals0, &vals1);

    col0.tree_prune_last(&keep);
    col1.tree_prune_last(&keep);

    let s0 = col0.final_shares();
    let s1 = col1.final_shares();

    for res in &KeyCollection::<FieldElm,fastfield::FE>::final_values(&s0, &s1) {
        println!("Path = {:?}", res.path);
        let s = crate::bits_to_string(&res.path);
        println!("Value: {:?} = {:?}", s, res.value);
    }
}


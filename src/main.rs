use counttree::collect;
use counttree::fastfield::FE;
use counttree::mpc;
use counttree::prg;
use counttree::sketch;
//use counttree::FieldElm;
//use rand_core::RngCore;
//use crypto::util;

use std::env;
//use rand::random;

fn verify_sketches(
    col0: &mut collect::KeyCollection<FE,FE>,
    col1: &mut collect::KeyCollection<FE,FE>,
    level: usize,
) {
    let (start, end) = (0, col0.keys.len());

    //println!("   frontier");
    let sketch0 = col0.tree_sketch_frontier(start, end);
    let sketch1 = col1.tree_sketch_frontier(start, end);
    //println!("   done");

    //println!("   mul");
    let mut triples0 = vec![];
    let mut triples1 = vec![];

    let mut mac0 = vec![];
    let mut mac1= vec![];

    let mut macp0 = vec![];
    let mut macp1= vec![];

    for key in &col0.keys {
       triples0.push(key.1.triples.clone()); 
       mac0.push(key.1.mac_key); 
       macp0.push(key.1.mac_key2); 
    }

    for key in &col1.keys {
       triples1.push(key.1.triples.clone()); 
       mac1.push(key.1.mac_key); 
       macp1.push(key.1.mac_key2); 
    }

    let many_mul0 = mpc::ManyMulState::new(false, &triples0, &mac0, &macp0, &sketch0, level);
    let many_mul1 = mpc::ManyMulState::new(true, &triples1, &mac1, &macp1, &sketch1, level);

    let cor_shares0 = many_mul0.cor_shares();
    let cor_shares1 = many_mul1.cor_shares();

    let cor = mpc::ManyMulState::cors(&cor_shares0, &cor_shares1);

    let out_shares0 = many_mul0.out_shares(&cor);
    let out_shares1 = many_mul1.out_shares(&cor);

    let out = mpc::ManyMulState::verify(&out_shares0, &out_shares1);

    /*
    if out.len() > 7 {
        println!("Causing key 3 and 7 to be removed");
        out[3] = false;
       // out[7] = false;
    }*/

    col0.apply_sketch_results(&out);
    col1.apply_sketch_results(&out);

    println!("   done");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        panic!("Usage: {:?} <nclients>", args[0]);
    }

    let nclients = args[1].parse().unwrap();

    let mut client_strings = vec![];
    for i in 0..100 {
        let v1 = rand::random::<u32>();
        let v2 = rand::random::<u32>();
        println!("String[{:?}] = {:?}{:?}", i, v1, v2);
        client_strings.push(format!("{:016}{:016}", v1, v2));
    }

    let strlen = counttree::string_to_bits(&client_strings[0]).len();

    let seed = prg::PrgSeed::random();
    let mut col0 = collect::KeyCollection::<FE,FE>::new(&seed, strlen);
    let mut col1 = collect::KeyCollection::<FE,FE>::new(&seed, strlen);

    let mut keys = vec![];
    println!("Starting to generate keys");
    for s in &client_strings {
        keys.push(sketch::SketchDPFKey::gen_from_str(&s));
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

    //PROFILER.lock().unwrap().start("./sketch-2.profile").unwrap();
    let threshold = FE::from(2u32);
    let mut bad = 0;
    for level in 0..strlen {
        println!("...crawl {:?}", bad);
        let vals0 = col0.tree_crawl();
        let vals1 = col1.tree_crawl();
        //println!("...done");

        assert_eq!(vals0.len(), vals1.len());

        /*
        for v in vals0.iter_mut() {
            *v += 123123;
            *v %= 9223372036854775783u64;
        }
        */
        /*
        if bad == 2 {
            println!("Corrupt value");
            vals0[0] += 123123;
        }*/

        println!("Starting to sketch");
        verify_sketches(&mut col0, &mut col1, level);
        println!("Done");

        let keep = collect::KeyCollection::<FE,FE>::keep_values(nclients, &threshold, &vals0, &vals1);

        col0.tree_prune(&keep);
        col1.tree_prune(&keep);

        bad += 1;
    }
    //PROFILER.lock().unwrap().stop().unwrap();

    let s0 = col0.final_shares();
    let s1 = col1.final_shares();

    for res in &collect::KeyCollection::<FE,FE>::final_values(&s0, &s1) {
        println!("Path = {:?}", res.path);
        let s = counttree::bits_to_string(&res.path);
        println!("Value: {:?} = {:?}", s, res.value.value());
    }
}

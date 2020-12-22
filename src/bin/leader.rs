use counttree::{
    FieldElm,
    collect, config, fastfield, mpc,
    rpc::{
        AddKeysRequest, FinalSharesRequest, ResetRequest, 
        TreeInitRequest,
        TreeCrawlRequest, 
        TreeCrawlLastRequest, 
        TreeOutSharesRequest, 
        TreeOutSharesLastRequest, 
        TreePruneRequest, 
        TreePruneLastRequest, 
        TreeSketchFrontierRequest,
        TreeSketchFrontierLastRequest,
    },
    sketch,
};

use std::time::Instant;

use futures::try_join;
use std::io;

use rand::Rng;
use rayon::prelude::*;
use tarpc::{
    client,
    context,
    //server::{self, Channel},
};

use rand::distributions::Alphanumeric;
use tokio::net::TcpStream;
use tokio_serde::formats::Bincode;

use std::time::{Duration, SystemTime};

type SketchKey = sketch::SketchDPFKey<fastfield::FE,FieldElm>;

fn long_context() -> context::Context {
    let mut ctx = context::current();

    // Increase timeout to one hour
    ctx.deadline = SystemTime::now() + Duration::from_secs(1000000);
    ctx
}

fn sample_string(len: usize) -> String {
    let mut rng = rand::thread_rng();
    std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .take(len / 8)
        .collect()
}

fn generate_keys(cfg: &config::Config) -> (Vec<SketchKey>, Vec<SketchKey>) {
    let (keys0, keys1): (Vec<SketchKey>, Vec<SketchKey>) = rayon::iter::repeat(0)
        .take(cfg.num_sites)
        .map(|_| {
            let data_string = sample_string(cfg.data_len);
            let keys = sketch::SketchDPFKey::gen_from_str(&data_string);

            // XXX remove these clones
            (keys[0].clone(), keys[1].clone())
        })
        .unzip();

    let encoded: Vec<u8> = bincode::serialize(&keys0[0]).unwrap();
    println!("Key size: {:?} bytes", encoded.len());

    (keys0, keys1)
}

async fn reset_servers(
    client0: &mut counttree::CollectorClient,
    client1: &mut counttree::CollectorClient,
) -> io::Result<()> {
    let req = ResetRequest {};
    let response0 = client0.reset(long_context(), req.clone());
    let response1 = client1.reset(long_context(), req);
    try_join!(response0, response1).unwrap();

    Ok(())
}

async fn tree_init(
    client0: &mut counttree::CollectorClient,
    client1: &mut counttree::CollectorClient,
) -> io::Result<()> {
    let req = TreeInitRequest {};
    let response0 = client0.tree_init(long_context(), req.clone());
    let response1 = client1.tree_init(long_context(), req);
    try_join!(response0, response1).unwrap();

    Ok(())
}

async fn add_keys(
    cfg: &config::Config,
    mut client0: counttree::CollectorClient,
    mut client1: counttree::CollectorClient,
    keys0: &[sketch::SketchDPFKey<fastfield::FE,FieldElm>],
    keys1: &[sketch::SketchDPFKey<fastfield::FE,FieldElm>],
    nreqs: usize,
) -> io::Result<()> {
    use rand::distributions::Distribution;
    let mut rng = rand::thread_rng();
    let zipf = zipf::ZipfDistribution::new(cfg.num_sites, cfg.zipf_exponent).unwrap();

    let mut addkey0 = Vec::with_capacity(nreqs);
    let mut addkey1 = Vec::with_capacity(nreqs);

    for _j in 0..nreqs {
        let sample = zipf.sample(&mut rng) - 1;
        addkey0.push(keys0[sample].clone());
        addkey1.push(keys1[sample].clone());
    }

    let req0 = AddKeysRequest { keys: addkey0 };
    let req1 = AddKeysRequest { keys: addkey1 };

    let response0 = client0.add_keys(long_context(), req0.clone());
    let response1 = client1.add_keys(long_context(), req1.clone());

    try_join!(response0, response1).unwrap();

    Ok(())
}

async fn verify_sketches(
    client0: &mut counttree::CollectorClient,
    client1: &mut counttree::CollectorClient,
    level: usize,
    start: usize,
    end: usize,
) -> io::Result<Vec<bool>> {
    // Cor shares
    let req = TreeSketchFrontierRequest { level, start, end };
    let response0 = client0.tree_sketch_frontier(long_context(), req.clone());
    let response1 = client1.tree_sketch_frontier(long_context(), req);
    let (cor_shares0, cor_shares1) = try_join!(response0, response1).unwrap();
    let cor = mpc::ManyMulState::cors(&cor_shares0, &cor_shares1);

    // Out shares
    let req = TreeOutSharesRequest { cor };
    let response0 = client0.tree_out_shares(long_context(), req.clone());
    let response1 = client1.tree_out_shares(long_context(), req);
    let (out_shares0, out_shares1) = try_join!(response0, response1).unwrap();

    Ok(mpc::ManyMulState::verify(&out_shares0, &out_shares1))
}

async fn verify_sketches_last(
    client0: &mut counttree::CollectorClient,
    client1: &mut counttree::CollectorClient,
    start: usize,
    end: usize,
) -> io::Result<Vec<bool>> {
    // Cor shares
    let req = TreeSketchFrontierLastRequest { start, end };
    let response0 = client0.tree_sketch_frontier_last(long_context(), req.clone());
    let response1 = client1.tree_sketch_frontier_last(long_context(), req);
    let (cor_shares0, cor_shares1) = try_join!(response0, response1).unwrap();
    let cor = mpc::ManyMulState::cors(&cor_shares0, &cor_shares1);

    // Out shares
    let req = TreeOutSharesLastRequest { cor };
    let response0 = client0.tree_out_shares_last(long_context(), req.clone());
    let response1 = client1.tree_out_shares_last(long_context(), req);
    let (out_shares0, out_shares1) = try_join!(response0, response1).unwrap();

    Ok(mpc::ManyMulState::verify(&out_shares0, &out_shares1))
}

async fn run_level(
    cfg: &config::Config,
    client0: &mut counttree::CollectorClient,
    client1: &mut counttree::CollectorClient,
    level: usize,
    nreqs: usize,
    start_time: Instant,
) -> io::Result<usize> {
    let threshold64 = core::cmp::max(1, (cfg.threshold * (nreqs as f64)) as u64);
    let threshold = fastfield::FE::new(threshold64);

    // Tree crawl
    println!(
        "TreeCrawlStart {:?} {:?} {:?}",
        level,
        "-",
        start_time.elapsed().as_secs_f64()
    );
    let req = TreeCrawlRequest {};
    let response0 = client0.tree_crawl(long_context(), req.clone());
    let response1 = client1.tree_crawl(long_context(), req);
    let (vals0, vals1) = try_join!(response0, response1).unwrap();
    println!(
        "TreeCrawlDone {:?} {:?} {:?}",
        level,
        "-",
        start_time.elapsed().as_secs_f64()
    );

    println!(
        "SketchStart {:?} {:?} {:?}",
        level,
        "-",
        start_time.elapsed().as_secs_f64()
    );

    let sketch_start = Instant::now();

    // Run sketching in chunks of cfg.sketch_batch_size to avoid having huge RPC messages.
    let mut start = 0;
    while start < nreqs {
        let end = std::cmp::min(nreqs, start + cfg.sketch_batch_size);
        let out = verify_sketches(client0, client1, level, start, end).await?;
        start += cfg.sketch_batch_size;

        for v in out {
            assert!(v);
        }
    }

    println!(
        "SketchDone {:?} {:?} {:?} rate={:?}",
        level,
        "-",
        start_time.elapsed().as_secs_f64(),
        (nreqs as f64) / sketch_start.elapsed().as_secs_f64()
    );

    assert_eq!(vals0.len(), vals1.len());
    let keep = collect::KeyCollection::<fastfield::FE,FieldElm>::keep_values(nreqs, &threshold, &vals0, &vals1);
    //println!("Keep: {:?}", keep);
    //println!("KeepLen: {:?}", keep.len());

    // Tree prune
    let req = TreePruneRequest { keep };
    let response0 = client0.tree_prune(long_context(), req.clone());
    let response1 = client1.tree_prune(long_context(), req);
    try_join!(response0, response1).unwrap();

    Ok(vals0.len())
}

async fn run_level_last(
    cfg: &config::Config,
    client0: &mut counttree::CollectorClient,
    client1: &mut counttree::CollectorClient,
    nreqs: usize,
    start_time: Instant,
) -> io::Result<usize> {
    let threshold64 = core::cmp::max(1, (cfg.threshold * (nreqs as f64)) as u32);
    let threshold = FieldElm::from(threshold64);

    // Tree crawl
    println!(
        "TreeCrawlStart last {:?} {:?}",
        "-",
        start_time.elapsed().as_secs_f64()
    );
    let req = TreeCrawlLastRequest {};
    let response0 = client0.tree_crawl_last(long_context(), req.clone());
    let response1 = client1.tree_crawl_last(long_context(), req);
    let (vals0, vals1) = try_join!(response0, response1).unwrap();
    println!(
        "TreeCrawlDone last {:?} {:?}",
        "-",
        start_time.elapsed().as_secs_f64()
    );

    println!(
        "SketchStart last {:?} {:?}",
        "-",
        start_time.elapsed().as_secs_f64()
    );

    let sketch_start = Instant::now();

    // Run sketching in chunks of cfg.sketch_batch_size to avoid having huge RPC messages.
    let mut start = 0;
    while start < nreqs {
        let end = std::cmp::min(nreqs, start + cfg.sketch_batch_size_last);
        let out = verify_sketches_last(client0, client1, start, end).await?;
        start += cfg.sketch_batch_size_last;

        for v in out {
            assert!(v);
        }
    }

    println!(
        "SketchDone last {:?} {:?} rate={:?}",
        "-",
        start_time.elapsed().as_secs_f64(),
        (nreqs as f64) / sketch_start.elapsed().as_secs_f64()
    );

    assert_eq!(vals0.len(), vals1.len());
    let keep = collect::KeyCollection::<fastfield::FE,FieldElm>::keep_values_last(nreqs, &threshold, &vals0, &vals1);
    //println!("Keep: {:?}", keep);
    //println!("KeepLen: {:?}", keep.len());

    // Tree prune
    let req = TreePruneLastRequest { keep };
    let response0 = client0.tree_prune_last(long_context(), req.clone());
    let response1 = client1.tree_prune_last(long_context(), req);
    try_join!(response0, response1).unwrap();

    Ok(vals0.len())
}

async fn final_shares(
    client0: &mut counttree::CollectorClient,
    client1: &mut counttree::CollectorClient,
) -> io::Result<()> {
    // Final shares
    let req = FinalSharesRequest {};
    let response0 = client0.final_shares(long_context(), req.clone());
    let response1 = client1.final_shares(long_context(), req);
    try_join!(response0, response1).unwrap();

    /*
    for res in &collect::KeyCollection::<fastfield::FE,FieldElm>::final_values(&vals0, &vals1) {
        println!("Path = {:?}", res.path);
        let s = crate::bits_to_string(&res.path);
        println!("Value: {:?} = {:?}", s, res.value);
    }*/

    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    //println!("Using only one thread!");
    //rayon::ThreadPoolBuilder::new().num_threads(1).build_global().unwrap();

    env_logger::init();
    let (cfg, _, nreqs) = config::get_args("Leader", false, true);
    debug_assert_eq!(cfg.data_len % 8, 0);

    let mut builder = native_tls::TlsConnector::builder();
    // XXX THERE IS NO CERTIFICATE VALIDATION HERE!!!
    // This is just for benchmarking purposes. A real implementation would
    // have pinned certificates for the two servers and use those to encrypt.
    builder.danger_accept_invalid_certs(true);

    let cx = builder.build().unwrap();
    let cx = tokio_native_tls::TlsConnector::from(cx);

    let tcp0 = TcpStream::connect(cfg.server0).await?;
    let io0 = cx.connect("server0", tcp0).await.unwrap();
    let transport0 = tarpc::serde_transport::Transport::from((io0, Bincode::default()));

    let tcp1 = TcpStream::connect(cfg.server1).await?;
    let io1 = cx.connect("server1", tcp1).await.unwrap();
    let transport1 = tarpc::serde_transport::Transport::from((io1, Bincode::default()));

    //let transport0 = tarpc::serde_transport::tcp::connect(cfg.server0, Bincode::default()).await?;

    let mut client0 =
        counttree::CollectorClient::new(client::Config::default(), transport0).spawn()?;
    let mut client1 =
        counttree::CollectorClient::new(client::Config::default(), transport1).spawn()?;

    let start = Instant::now();
    let (keys0, keys1) = generate_keys(&cfg);
    let delta = start.elapsed().as_secs_f64();
    println!(
        "Generated {:?} keys in {:?} seconds ({:?} sec/key)",
        keys0.len(),
        delta,
        delta / (keys0.len() as f64)
    );

    reset_servers(&mut client0, &mut client1).await?;

    let mut left_to_go = nreqs;
    let reqs_in_flight = 1000;
    while left_to_go > 0 {
        let mut resps = vec![];

        for _j in 0..reqs_in_flight {
            let this_batch = std::cmp::min(left_to_go, cfg.addkey_batch_size);
            left_to_go -= this_batch;

            if this_batch > 0 {
                resps.push(add_keys(
                    &cfg,
                    client0.clone(),
                    client1.clone(),
                    &keys0,
                    &keys1,
                    this_batch,
                ));
            }
        }

        for r in resps {
            r.await?;
        }
    }

    tree_init(&mut client0, &mut client1).await?;

    let start = Instant::now();
    for level in 0..cfg.data_len-1 {
        let active_paths = run_level(&cfg, &mut client0, &mut client1, level, nreqs, start).await?;

        println!(
            "Level {:?} active_paths={:?} {:?}",
            level,
            active_paths,
            start.elapsed().as_secs_f64()
        );
    }

    let active_paths = run_level_last(&cfg, &mut client0, &mut client1, nreqs, start).await?;
    println!(
        "Level {:?} active_paths={:?} {:?}",
        cfg.data_len,
        active_paths,
        start.elapsed().as_secs_f64()
    );

    final_shares(&mut client0, &mut client1).await?;

    Ok(())
}

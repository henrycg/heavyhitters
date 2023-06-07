// Starter code from:
//   https://github.com/google/tarpc/blob/master/example-service/src/server.rs

use counttree::{
    collect, config,
    FieldElm,
    fastfield::FE,
    mpc, prg,
    rpc::Collector,
    rpc::{
        AddKeysRequest, FinalSharesRequest, ResetRequest, TreeCrawlRequest, 
        TreeCrawlLastRequest, TreeInitRequest,
        TreeOutSharesRequest, 
        TreeOutSharesLastRequest, 
        TreePruneRequest, 
        TreePruneLastRequest, 
        TreeSketchFrontierRequest,
        TreeSketchFrontierLastRequest,
    },
};

use futures::{
    future::{self, Ready},
    prelude::*,
};
use std::{
    io,
    sync::{Arc, Mutex},
};
use tarpc::{
    context,
    server::{self, Channel},
    tokio_serde::formats::Bincode,
    serde_transport::tcp,
};

#[derive(Clone)]
struct CollectorServer {
    seed: prg::PrgSeed,
    data_len: usize,
    server_idx: u16,
    arc: Arc<Mutex<collect::KeyCollection<FE,FieldElm>>>,
    arc_mul: Arc<Mutex<mpc::ManyMulState<FE>>>,
    arc_mul_last: Arc<Mutex<mpc::ManyMulState<FieldElm>>>,
}

impl Collector for CollectorServer {
    type AddKeysFut = Ready<String>;
    type TreeInitFut = Ready<String>;
    type TreeCrawlFut = Ready<Vec<FE>>;
    type TreeCrawlLastFut = Ready<Vec<FieldElm>>;
    type TreePruneFut = Ready<String>;
    type TreePruneLastFut = Ready<String>;
    type TreeSketchFrontierFut = Ready<mpc::ManyCorShare<FE>>;
    type TreeSketchFrontierLastFut = Ready<mpc::ManyCorShare<FieldElm>>;
    type TreeOutSharesFut = Ready<mpc::ManyOutShare<FE>>;
    type TreeOutSharesLastFut = Ready<mpc::ManyOutShare<FieldElm>>;
    type FinalSharesFut = Ready<Vec<collect::Result<FieldElm>>>;
    type ResetFut = Ready<String>;

    fn reset(self, _: context::Context, _rst: ResetRequest) -> Self::ResetFut {
        let mut coll = self.arc.lock().unwrap();
        *coll = collect::KeyCollection::new(&self.seed, self.data_len);
        *self.arc_mul.lock().unwrap() = mpc::ManyMulState::zero();
        *self.arc_mul_last.lock().unwrap() = mpc::ManyMulState::zero();

        future::ready("Done".to_string())
    }

    fn add_keys(self, _: context::Context, add: AddKeysRequest) -> Self::AddKeysFut {
        let mut coll = self.arc.lock().unwrap();
        for k in add.keys {
            coll.add_key(k);
        }
        println!("Number of keys: {:?}", coll.keys.len());

        future::ready("".to_string())
    }

    fn tree_init(self, _: context::Context, _req: TreeInitRequest) -> Self::TreeInitFut {
        let mut coll = self.arc.lock().unwrap();
        coll.tree_init();
        future::ready("Done".to_string())
    }

    fn tree_crawl(self, _: context::Context, _req: TreeCrawlRequest) -> Self::TreeCrawlFut {
        let mut coll = self.arc.lock().unwrap();
        future::ready(coll.tree_crawl())
    }

    fn tree_crawl_last(self, _: context::Context, _req: TreeCrawlLastRequest) -> Self::TreeCrawlLastFut {
        let mut coll = self.arc.lock().unwrap();
        future::ready(coll.tree_crawl_last())
    }

    fn tree_prune(self, _: context::Context, req: TreePruneRequest) -> Self::TreePruneFut {
        let mut coll = self.arc.lock().unwrap();
        coll.tree_prune(&req.keep);
        future::ready("Done".to_string())
    }

    fn tree_prune_last(self, _: context::Context, req: TreePruneLastRequest) -> Self::TreePruneLastFut {
        let mut coll = self.arc.lock().unwrap();
        coll.tree_prune_last(&req.keep);
        future::ready("Done".to_string())
    }

    fn tree_sketch_frontier(
        self,
        _: context::Context,
        req: TreeSketchFrontierRequest,
    ) -> Self::TreeSketchFrontierFut {
        let mut coll = self.arc.lock().unwrap();
        let sketch = coll.tree_sketch_frontier(req.start, req.end);

        let mut triples = vec![];
        let mut mac = vec![];
        let mut macp = vec![];

        for key in &coll.keys[req.start..req.end] {
            triples.push(key.1.triples.clone());
            mac.push(key.1.mac_key);
            macp.push(key.1.mac_key2);
        }

        let state = mpc::ManyMulState::new(self.server_idx > 0, 
                                           &triples, &mac, &macp,
                                           &sketch, 
                                           req.level);
        let cor_shares = state.cor_shares();
        *self.arc_mul.lock().unwrap() = state;

        future::ready(cor_shares)
    }

    fn tree_sketch_frontier_last(
        self,
        _: context::Context,
        req: TreeSketchFrontierLastRequest,
    ) -> Self::TreeSketchFrontierLastFut {
        let mut coll = self.arc.lock().unwrap();
        let sketch = coll.tree_sketch_frontier_last(req.start, req.end);

        let mut triples = vec![];
        let mut mac = vec![];
        let mut macp = vec![];

        for key in &coll.keys[req.start..req.end] {
            triples.push(key.1.triples_last.clone());
            mac.push(key.1.mac_key_last.clone());
            macp.push(key.1.mac_key2_last.clone());
        }

        let state = mpc::ManyMulState::new(self.server_idx > 0, 
                                           &triples, &mac, &macp,
                                           &sketch, 
                                           0);
        let cor_shares = state.cor_shares();
        *self.arc_mul_last.lock().unwrap() = state;

        future::ready(cor_shares)
    }

    fn tree_out_shares(
        self,
        _: context::Context,
        req: TreeOutSharesRequest,
    ) -> Self::TreeOutSharesFut {
        let state = self.arc_mul.lock().unwrap();
        let out = state.out_shares(&req.cor);

        future::ready(out)
    }

    fn tree_out_shares_last(
        self,
        _: context::Context,
        req: TreeOutSharesLastRequest,
    ) -> Self::TreeOutSharesLastFut {
        let state = self.arc_mul_last.lock().unwrap();
        let out = state.out_shares(&req.cor);

        future::ready(out)
    }

    fn final_shares(self, _: context::Context, _req: FinalSharesRequest) -> Self::FinalSharesFut {
        let coll = self.arc.lock().unwrap();
        let out = coll.final_shares();
        future::ready(out)
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let (cfg, sid, _) = config::get_args("Server", true, false);
    let server_addr = match sid {
        0 => cfg.server0,
        1 => cfg.server1,
        _ => panic!("Oh no!"),
    };

    let server_idx = match sid {
        0 => 0,
        1 => 1,
        _ => panic!("Oh no!"),
    };

    // XXX This is bogus
    let seed = prg::PrgSeed { key: [1u8; 16] };

    let coll = collect::KeyCollection::new(&seed, cfg.data_len);
    let arc = Arc::new(Mutex::new(coll));
    let arc_mul = Arc::new(Mutex::new(mpc::ManyMulState::zero()));
    let arc_mul_last = Arc::new(Mutex::new(mpc::ManyMulState::zero()));

    let mut server_addr = server_addr;
    // Listen on any IP
    server_addr.set_ip("0.0.0.0".parse().expect("Could not parse"));
    tcp::listen(&server_addr, Bincode::default)
        .await?
        // Ignore accept errors.
        .filter_map(|r| future::ready(r.ok()))
        .map(server::BaseChannel::with_defaults)
        .map(|channel| {
            let coll_server = CollectorServer {
                server_idx,
                seed: seed.clone(),
                data_len: cfg.data_len,
                arc: arc.clone(),
                arc_mul: arc_mul.clone(),
                arc_mul_last: arc_mul_last.clone(),
            };

            channel.execute(coll_server.serve())
        })
        .buffer_unordered(100)
        .for_each(|_| async {})
        .await;

    Ok(())
}

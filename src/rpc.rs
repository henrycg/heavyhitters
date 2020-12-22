use crate::collect;
use crate::FieldElm;
use crate::fastfield::FE;
use crate::mpc::{ManyCor, ManyCorShare, ManyOutShare};
use crate::sketch::SketchDPFKey;

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResetRequest {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddKeysRequest {
    pub keys: Vec<SketchDPFKey<FE,FieldElm>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeInitRequest {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeCrawlRequest {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeCrawlLastRequest {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreePruneRequest {
    pub keep: Vec<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreePruneLastRequest {
    pub keep: Vec<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeSketchFrontierRequest {
    pub level: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeSketchFrontierLastRequest {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeOutSharesRequest {
    pub cor: ManyCor<FE>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeOutSharesLastRequest {
    pub cor: ManyCor<FieldElm>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalSharesRequest {}

#[tarpc::service]
pub trait Collector {
    async fn reset(rst: ResetRequest) -> String;
    async fn add_keys(add: AddKeysRequest) -> String;
    async fn tree_init(req: TreeInitRequest) -> String;
    async fn tree_crawl(req: TreeCrawlRequest) -> Vec<FE>;
    async fn tree_crawl_last(req: TreeCrawlLastRequest) -> Vec<FieldElm>;
    async fn tree_prune(req: TreePruneRequest) -> String;
    async fn tree_prune_last(req: TreePruneLastRequest) -> String;
    async fn tree_sketch_frontier(req: TreeSketchFrontierRequest) -> ManyCorShare<FE>;
    async fn tree_sketch_frontier_last(req: TreeSketchFrontierLastRequest) -> ManyCorShare<FieldElm>;
    async fn tree_out_shares(req: TreeOutSharesRequest) -> ManyOutShare<FE>;
    async fn tree_out_shares_last(req: TreeOutSharesLastRequest) -> ManyOutShare<FieldElm>;
    async fn final_shares(req: FinalSharesRequest) -> Vec<collect::Result<FieldElm>>;
}

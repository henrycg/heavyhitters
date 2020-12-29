# README

## WARNING: This is not production-ready code.

This is software for a research prototype. Please
do *NOT* use this code in production.

## Background

This is the source code that accompanies the paper
>  "Lightweight Techniques for Private Heavy Hitters".  
> by Dan Boneh, Elette Boyle, Henry Corrigan-Gibbs, Niv Gilboa, and Yuval Ishai.  
> _IEEE Symposium on Security and Privacy 2021_

For questions about the code, please contact Henry at:  henrycg {at} csail {dot} mit {dot} edu.

We have tested this code with:
>  rustc 1.44.1 (c7087fe00 2020-06-17)  
>  rustc 1.50.0-nightly (11c94a197 2020-12-21)

## Getting started

First, make sure that you have a working Rust installation:

```
$ rustc --version   
rustc 1.47.0
$ cargo --version
cargo 1.46.0
```

Now run the following steps to build and test the source:

```
## Set the RUSTFLAGS environment variable to build
## with AES-NI and vector instructions where available.
## Make sure to build with the --release flag, otherwise
## the performance will be terrible. If you are using a
## non-bash shell, then you will have to modify the following
## command.
$ export RUSTFLAGS+="-C target-cpu=native" 
$ cargo build --release

## Run tets.
$ cargo test
... lots of output ...

```

You should now be set to run the code. In one shell, run the following command:

```
$ cargo run --release --bin server -- --config src/bin/config.json --server_id 0
```

This starts one server process with ID `0` using the config file located at `src/bin/config.json`. In a second shell, you can start the second server process:

```
$ cargo run --release --bin server -- --config src/bin/config.json --server_id 1
```

Now, the servers should be ready to process client requests. In a third shell, run the following command to send `1000` client requests to the servers (this will take some time):

```
$ cargo run --release --bin leader -- --config src/bin/config.json -n 1000
```

You should see lots of output...

## The config file

The client and servers use a common configuration file, which contains the parameters for the system. An example of one such file is in `src/bin/config.json`. The contents of that file are here:

```
{
  "data_len": 512,
  "threshold": 0.001,
  "server0": "0.0.0.0:8000",
  "server1": "0.0.0.0:8001",
  "addkey_batch_size": 100,
  "sketch_batch_size": 100000,
  "sketch_batch_size_last": 25000,
  "num_sites": 10000,
  "zipf_exponent": 1.03
}
```

The parameters are:

* `data_len`: The bitlength of each client's private string.
* `threshold`: The servers will output the collection of strings that more than a `threshold` of clients hold.
* `server0` and `server1`: The `IP:port` of tuple for the two servers. The servers can run on different IP addresses, but these IPs must be publicly addressable.
* `*_batch_size`: The number of each type of RPC request to bundle together. The underlying RPC library has an annoying limit on the size of each RPC request, so you cannot set these values too large.
* `num_sites` and `zipf_exponent`: Each simulated client samples its private string from a Zipf distribution over strings with parameter `zipf_exponent` and support `num_sites`.

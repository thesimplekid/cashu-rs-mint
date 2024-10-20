# Moved to https://github.com/cashubtc/cdk/tree/main/crates/cdk-mintd


# cashu-rs-mint

*Disclaimer: The author is NOT a cryptographer and this work has not been reviewed. This means that there is very likely a fatal flaw somewhere. Cashu is still experimental and not production-ready.*

WIP

## Implemented [NUTs](https://github.com/cashubtc/nuts/):

- :heavy_check_mark: [NUT-00](https://github.com/cashubtc/nuts/blob/main/00.md)
- :heavy_check_mark: [NUT-01](https://github.com/cashubtc/nuts/blob/main/01.md)
- :heavy_check_mark: [NUT-02](https://github.com/cashubtc/nuts/blob/main/02.md)
- :heavy_check_mark: [NUT-03](https://github.com/cashubtc/nuts/blob/main/03.md)
- :heavy_check_mark: [NUT-04](https://github.com/cashubtc/nuts/blob/main/04.md)
- :heavy_check_mark: [NUT-05](https://github.com/cashubtc/nuts/blob/main/05.md)
- :heavy_check_mark: [NUT-06](https://github.com/cashubtc/nuts/blob/main/06.md)
- :heavy_check_mark: [NUT-07](https://github.com/cashubtc/nuts/blob/main/07.md)
- :heavy_check_mark: [NUT-08](https://github.com/cashubtc/nuts/blob/main/08.md)
- :heavy_check_mark: [NUT-09](https://github.com/cashubtc/nuts/blob/main/09.md)


## Development

```
nix develop
```

This will launch a nix shell with a regtest bitcoind node as well as two lightning nodes.

In order to use the node first a channel will need to be opened.

```
  ln1 newaddr
  ln2 newaddr
```

```
  btc sendtoaddress <ln1 bitcoin address> 100
  btc sendtoaddress <ln2 bitcoin address> 100
  btc getnewaddress
  btc generatetoaddress 50 <btc address>
```

Connect ln nodes
```
  ln2 getinfo
  ln1 connect <pubkey of ln1> 127.0.0.1 15352
```

Open a channel from ln1 to ln2
```
  ln1 fundchannel id=<pubkey of ln2> amount=10000000
```

Open a channel from ln2 to ln1
```
  ln1 getinfo
  ln2 fundchannel id=<pubkey of ln1> amount=10000000
```

Generate blocks to confirm channels
```
  btc getnewaddress
  btc generatetoaddress 50 <btc address>
```

Start the mint, by default the mint will use ln1
```
  cargo r
```

## Implemented Lightning Backends
- :heavy_check_mark: [CLNrpc](https://github.com/ElementsProject/lightning#using-the-json-rpc-interface)
- :construction: [Greenlight](https://github.com/Blockstream/greenlight)
- :construction: [ldk-node](https://github.com/lightningdevkit/ldk-node)
 
## License

Code is under the [BSD 3-Clause License](LICENSE-BSD-3)

## Contribution

All contributions welcome.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, shall be licensed as above, without any additional terms or conditions.

## Contact

I can be contacted for comments or questions on nostr at _@thesimplekid.com (npub1qjgcmlpkeyl8mdkvp4s0xls4ytcux6my606tgfx9xttut907h0zs76lgjw) or via email tsk@thesimplekid.com.

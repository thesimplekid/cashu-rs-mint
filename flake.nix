{
  description = "A flake for developing cashu-rs-mint";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";

    flakebox = {
      url = "github:rustshop/flakebox";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flakebox, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        flakeboxLib = flakebox.lib.${system} { };
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells = flakeboxLib.mkShells {
          buildInputs = [
          pkgs.clightning
          pkgs.bitcoind
          ];
        shellHook = ''

        bitcoind -daemon -regtest
        bitcoin-cli -regtest -rpcport=4000 -rpcpassword=bitcoin -rpcuser=bitcoin createwallet "testwallet"
        address=`bitcoin-cli -regtest -rpcport=4000 -rpcpassword=bitcoin -rpcuser=bitcoin getnewaddress`
        bitcoin-cli -regtest -rpcport=4000 -rpcpassword=bitcoin -rpcuser=bitcoin generatetoaddress 50 $address
        bitcoin-cli -regtest -rpcport=4000 -rpcpassword=bitcoin -rpcuser=bitcoin getblockcount

        lightningd --daemon --network=regtest --lightning-dir=tmp/ln_1 --addr=127.0.0.1:19846 --autolisten=true --log-level=debug --log-file=./lig.log
        echo "Started first"
        lightningd --daemon --network=regtest --lightning-dir=tmp/ln_2 --addr=127.0.0.1:80888 --autolisten=true --log-level=debug --log-file=./lig_2.log

        alias btc="bitcoin-cli -regtest -rpcport=4000 -rpcpassword=bitcoin -rpcuser=bitcoin"
        alias ln1="lightning-cli --lightning-dir=tmp/ln_1 --regtest"
        alias ln2="lightning-cli --lightning-dir=tmp/ln_2 --regtest"

        '';
        };

      });
}

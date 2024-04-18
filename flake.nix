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
        bitcoind = pkgs.bitcoind.overrideDerivation (oldAttrs: { doCheck = false; doInstallCheck = false; });

        cashu_rs_mint_dir = "/tmp/cashu-rs-mint";
        bitcoin_dir = cashu_rs_mint_dir + "/bitcoin";
        lightning_dir = cashu_rs_mint_dir + "/lighting";
      in
      {
        devShells = flakeboxLib.mkShells {
          buildInputs = [
          pkgs.clightning
          bitcoind
          ];
        shellHook = ''

        mkdir -p ${cashu_rs_mint_dir}
        mkdir -p ${bitcoin_dir}
        mkdir -p ${lightning_dir}

        alias btc="${bitcoind}/bin/bitcoin-cli -regtest -datadir=${bitcoin_dir}"
        alias ln1="${pkgs.clightning}/bin/lightning-cli --lightning-dir=${lightning_dir}/ln_1 --regtest"
        alias ln2="${pkgs.clightning}/bin/lightning-cli --lightning-dir=${lightning_dir}/ln_2 --regtest"


        blockcount=$(btc getblockcount) || { blockcount=-1; }
        if [[ $blockcount == "-1" ]]; then
          echo "Starting bitcoind"
          bitcoind -regtest -datadir=${bitcoin_dir} -fallbackfee=0.01 -daemon
          sleep 1
        else
           echo "bitcoind already started"
        fi

        btc loadwallet "test" || btc createwallet "test" || echo "Wallet already loaded"

        address=`btc getnewaddress`
        btc generatetoaddress 50 $address

        ln_1_info=$(ln1 getinfo) || { ln_1_info=-1; }

        if [[ $ln_1_info == "-1" ]]; then
          echo "Starting ln1"
          lightningd --bitcoin-datadir=${bitcoin_dir} --network=regtest --lightning-dir=${lightning_dir}/ln_1 --addr=127.0.0.1:19846 --autolisten=true --log-level=debug --log-file=./debug.log --daemon
          sleep 1
        else
           echo "ln1 already started"
        fi

        ln_2_info=$(ln2 getinfo) || { ln_2_info=-1; }
        if [[ $ln_2_info == "-1" ]]; then
          echo "Starting ln2"
          lightningd --bitcoin-datadir=${bitcoin_dir} --network=regtest --lightning-dir=${lightning_dir}/ln_2 --addr=127.0.0.1:80888 --autolisten=true --log-level=debug --log-file=./debug.log --daemon
          sleep 1
        else
           echo "ln2 already started"
        fi



        '';
        };

      });
}

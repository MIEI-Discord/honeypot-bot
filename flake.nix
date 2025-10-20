{
  description = "A bot to assist with setting up a honeypot channel in a Discord Server (to automatically catch and silence/ban compromised accounts that spam post phishing links)";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    rust-flake = {
      url = "github:juspay/rust-flake";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake {inherit inputs;} {
      systems = inputs.flake-parts.inputs.nixpkgs-lib.lib.systems.flakeExposed;

      imports = [
        inputs.flake-parts.flakeModules.partitions
        inputs.rust-flake.flakeModules.default
        inputs.rust-flake.flakeModules.nixpkgs
      ];

      partitions = {
        dev = {
          extraInputsFlake = ./dev-flake;
          module = {
            imports = [
              ./dev-flake/flake-module.nix
            ];
          };
        };
      };

      partitionedAttrs = {
        checks = "dev";
        devShells = "dev";
        formatter = "dev";
      };

      perSystem = _: {
        rust-project = {
        };
      };
    };
}

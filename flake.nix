{
  description = "A bot to assist with setting up a honeypot channel in a Discord Server (to automatically catch and silence/ban compromised accounts that spam post phishing links)";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    rust-flake = {
      url = "github:juspay/rust-flake";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    actions-nix.url = "github:nialov/actions.nix";
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake {inherit inputs;} {
      systems = inputs.flake-parts.inputs.nixpkgs-lib.lib.systems.flakeExposed;

      imports = [
        inputs.flake-parts.flakeModules.partitions
        inputs.rust-flake.flakeModules.default
        inputs.rust-flake.flakeModules.nixpkgs
        inputs.actions-nix.flakeModules.default
        ./windows-tmp.nix
        ./ci
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

      perSystem = {
        config,
        lib,
        ...
      }: let
        inherit (config.rust-project) crane-lib;
        crate-cfg = config.rust-project.crates.honeypot-bot;

        common =
          crate-cfg.crane.args
          // {
            inherit (config.rust-project) src;
            pname = "honeypot-bot";
            cargoExtraArgs = "-p honeypot-bot";
            inherit (crate-cfg.cargoToml.package) version;
            strictDeps = true;
          };

        artifactsDev = crane-lib.buildDepsOnly (common
          // {
            cargoExtraArgs = "--locked --features dev_env";
            CARGO_PROFILE = "dev";
          });

        clippyDev = crane-lib.cargoClippy (common
          // {
            cargoArtifacts = artifactsDev;
            cargoExtraArgs = "--locked --features dev_env";
            CARGO_PROFILE = "dev";
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            meta =
              common.meta or {}
              // {
                description = "Clippy check for the honeypot-bot crate (with debug profile and `dev_env` feature enabled)";
              };
          });

        clippyRelease = crane-lib.cargoClippy (common
          // {
            inherit (crate-cfg.crane.outputs.drv.clippy) cargoArtifacts;
            cargoExtraArgs = "--locked";
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            meta =
              common.meta or {}
              // {
                description = "Clippy check for the honeypot-bot crate";
              };
          });
      in {
        rust-project = {
          crates = {
            honeypot-bot = {
              autoWire = lib.mkForce ["crate"];

              crane.outputs = {
                checks = {
                  honeypot-bot-clippy = clippyRelease;
                  honeypot-bot-debug-clippy = clippyDev;
                };
              };
            };
          };
        };

        packages = {
          default = config.packages.honeypot-bot;
        };
      };
    };
}

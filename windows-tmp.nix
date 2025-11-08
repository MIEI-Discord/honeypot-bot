{
  inputs,
  self,
  withSystem,
  ...
}: {
  flake = {
    # Hardcode Windows for now to get this over with; TODO using cross-parts when it's fixed
    packages.x86_64-windows = let
      # TODO: eventually complete `cross-parts` and use that instead of this ad-hoc solution from https://github.com/rsform/jacquard/blob/b19347c6ea894f92e610c2ff31419e15604f65e6/nix/modules/cross.nix and https://crane.dev/examples/cross-windows.html
      pkgsX = import inputs.nixpkgs {
        overlays = [(import inputs.rust-flake.inputs.rust-overlay)];
        localSystem = "x86_64-linux";
        crossSystem = {
          config = "x86_64-w64-mingw32";
          libc = "msvcrt";
        };
      };

      craneLibX = (inputs.rust-flake.inputs.crane.mkLib pkgsX).overrideToolchain (p:
        (p.rust-bin.fromRustupToolchainFile (self + /rust-toolchain.toml)).override {
          targets = ["x86_64-pc-windows-gnu"];
        });

      crate-cfg = withSystem "x86_64-linux" ({config, ...}: config.rust-project.crates.honeypot-bot);

      src = withSystem "x86_64-linux" ({config, ...}: config.rust-project.src);

      common =
        crate-cfg.crane.args
        // {
          inherit src;
          pname = "honeypot-bot";
          cargoExtraArgs = "-p honeypot-bot";
          inherit (crate-cfg.cargoToml.package) version;
          strictDeps = true;
        };

      honeypot-bot-windows-deps = craneLibX.buildDepsOnly common;
      honeypot-bot-windows = craneLibX.buildPackage (common
        // {
          cargoArtifacts = honeypot-bot-windows-deps;
        });
    in rec {
      honeypot-bot = honeypot-bot-windows;
      default = honeypot-bot;
    };
  };
}

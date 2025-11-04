{inputs, ...}: {
  imports = [
    inputs.treefmt-nix.flakeModule
  ];

  perSystem = {
    inputs',
    self',
    pkgs,
    config,
    ...
  }: let
    nixd-git = inputs'.nixd.packages.default;
    pinix-git = inputs'.pinix.packages.default;
    jujutsu-git = inputs'.jujutsu.packages.default;
    helix-git = inputs'.helix.packages.default;

    updateFlakes = pkgs.writeShellApplication {
      name = "update-flakes";

      runtimeInputs = [
        pkgs.fd
      ];

      text = ''
        fd -E templates flake.nix -x nix flake update --flake \{//\}
      '';
    };
  in {
    treefmt = {
      config = {
        projectRootFile = "flake.nix";

        programs = {
          alejandra.enable = true;
          statix.enable = true;
          deadnix.enable = true;

          taplo.enable = true;

          rustfmt = {
            enable = true;
            package = config.rust-project.toolchain;
          };
        };
      };
    };

    devShells = {
      default = pkgs.mkShellNoCC {
        inputsFrom = [
          self'.devShells.rust
        ];

        nativeBuildInputs =
          [
            config.treefmt.build.wrapper
            nixd-git
            pinix-git
            jujutsu-git
            helix-git
            updateFlakes
            pkgs.zed-editor-fhs
            pkgs.zellij
          ]
          ++ builtins.attrValues config.treefmt.build.programs;
      };
    };
  };
}

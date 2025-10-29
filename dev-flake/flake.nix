{
  description = "Flake module that defines development dependencies for `honeypot-bot`";

  inputs = {
    treefmt-nix.url = "github:numtide/treefmt-nix";
    nixd.url = "github:nix-community/nixd";
    pinix.url = "github:remi-dupre/pinix";
    jujutsu.url = "github:jj-vcs/jj";
    helix.url = "github:helix-editor/helix";
  };

  outputs = _: {};
}

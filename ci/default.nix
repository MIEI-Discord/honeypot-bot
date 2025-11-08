_: {
  flake = {
    actions-nix = {
      pre-commit.enable = true;

      workflows = {
        ".github/workflows/ci.yaml" = {
          on = {
            push = {
              branches = ["main"];

              paths = ["**.nix" "**.rs" "**.toml" "**.lock"];
            };
            # Trigger workflow manually
            workflow_dispatch = {};
            pull_request = {};
          };

          jobs = {
            nix-flake-check = {
              steps = [
                {
                  uses = "actions/checkout@v5";
                }
                {
                  uses = "cachix/install-nix-action@v31";
                }
                {
                  name = "Run `nix flake check`";
                  run = "nix flake check --show-trace -Lv";
                }
              ];
            };
          };
        };
      };
    };
  };
}

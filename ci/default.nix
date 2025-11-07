{inputs, ...}: {
  imports = [
    inputs.actions-nix.flakeModules.default
  ];

  flake = {
    actions-nix = {
      pre-commit.enable = true;

      workflows = {
        ".github/workflows/main.yaml" = {
          on = {
            push.branches = ["main"];
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

{
  description = "Sandbox environment for Claude Code in llmwiki-tooling";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    llm-agents = {
      url = "github:numtide/llm-agents.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, flake-utils, llm-agents, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        llm-pkgs = llm-agents.packages.${system};
        rust-toolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-analyzer" "clippy" "rustfmt" ];
        };
      in {
        packages.default = pkgs.buildFHSEnv {
          name = "llmwiki-tooling-env";
          targetPkgs = pkgs: with pkgs; [
            llm-pkgs.claude-code
            llm-pkgs.pi
            ripgrep
            ast-grep
            git
            cacert
            rust-toolchain
            gcc
            pkg-config
            openssl
            openssl.dev
          ];
          profile = ''
            export LANG="en_US.UTF-8"
            export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            export NIX_SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          '';
          runScript = "bash";
        };
      });
}
